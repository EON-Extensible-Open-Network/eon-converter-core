// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Reading PDF with PDFium: its text, its text as Markdown, its pages as pictures.
//!
//! A PDF describes where glyphs go on a page, not what is a heading or a list.
//! Markdown is rebuilt from what the page shows: lines from glyph positions,
//! headings from type larger than the body's, lists from their markers, words
//! split at a line end joined again, and the running headers and page numbers
//! that repeat on every page left out. Tables and columns come back as text in
//! reading order. A scanned PDF has no text to read.

use super::{pairs, Edge, Step};
use crate::format::Format;
use crate::tools::Engine;
use crate::Error;
use pdfium_render::prelude::*;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub fn edges(all: &mut Vec<Edge>) {
    pairs(all, Engine::Pdfium, &["pdf"], &["txt"], 3, |_, _| true);
    pairs(all, Engine::Pdfium, &["pdf"], &["md"], 3, |_, _| true);
    // One picture per page: a final step only (several pages make a folder).
    pairs(all, Engine::Pdfium, &["pdf"], &["png", "jpg"], 4, |_, _| {
        true
    });
}

/// PDFium is loaded once per process.
static PDFIUM: OnceLock<Result<Pdfium, String>> = OnceLock::new();

fn pdfium(library: &Path) -> Result<&'static Pdfium, Error> {
    PDFIUM
        .get_or_init(|| {
            Pdfium::bind_to_library(library)
                .map(Pdfium::new)
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| Error::Engine(format!("PDFium could not be loaded: {e}")))
}

pub async fn convert(
    step: &mut Step<'_>,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    let library = step.engine(Engine::Pdfium).await?;
    let (to, input, output) = (to.id, input.to_path_buf(), output.to_path_buf());
    let quality = step.options.jpeg_quality();
    (step.report)(0.0);
    tokio::task::spawn_blocking(move || {
        let pdfium = pdfium(&library)?;
        let doc = pdfium
            .load_pdf_from_file(&input, None)
            .map_err(|e| Error::Invalid(format!("this PDF could not be opened: {e}")))?;
        match to {
            "txt" => std::fs::write(&output, plain(&doc)?).map_err(|e| Error::Io(e.to_string())),
            "md" => std::fs::write(&output, markdown(&lines(&doc)?))
                .map_err(|e| Error::Io(e.to_string())),
            _ => pages_as_images(&doc, to, &output, quality),
        }
    })
    .await
    .map_err(|e| Error::Io(e.to_string()))?
}

fn plain(doc: &PdfDocument<'_>) -> Result<String, Error> {
    let mut out = String::new();
    for page in doc.pages().iter() {
        let text = page.text().map_err(|e| Error::Engine(e.to_string()))?;
        out.push_str(text.all().replace("\r\n", "\n").trim_end());
        out.push_str("\n\n");
    }
    if out.trim().is_empty() {
        return Err(no_text());
    }
    Ok(out)
}

fn no_text() -> Error {
    Error::Unsupported(
        "this PDF has no text in it (a scan?); reading it needs OCR, which is not built in yet"
            .into(),
    )
}

/// A line as the page shows it.
#[derive(Debug, Clone)]
pub struct Line {
    pub page: usize,
    pub text: String,
    pub size: f32,
    /// Distance from the top of the page, in points.
    pub top: f32,
    pub bottom: f32,
}

fn lines(doc: &PdfDocument<'_>) -> Result<Vec<Line>, Error> {
    let mut out = Vec::new();
    for (index, page) in doc.pages().iter().enumerate() {
        let height = page.height().value;
        let text = page.text().map_err(|e| Error::Engine(e.to_string()))?;
        let mut current: Option<Line> = None;
        let mut sizes: Vec<f32> = Vec::new();
        let flush = |line: Option<Line>, sizes: &mut Vec<f32>, out: &mut Vec<Line>| {
            if let Some(mut l) = line {
                if !l.text.trim().is_empty() {
                    sizes.sort_by(f32::total_cmp);
                    l.size = sizes.get(sizes.len() / 2).copied().unwrap_or(l.size);
                    l.text = l.text.trim().to_string();
                    out.push(l);
                }
            }
            sizes.clear();
        };
        for c in text.chars().iter() {
            let Some(s) = c.unicode_string() else {
                continue;
            };
            if s == "\r" || s == "\n" || s == "\r\n" {
                flush(current.take(), &mut sizes, &mut out);
                continue;
            }
            let size = c.unscaled_font_size().value;
            let Ok(b) = c.loose_bounds() else { continue };
            let (top, bottom) = (height - b.top().value, height - b.bottom().value);
            let same_line = current
                .as_ref()
                .is_some_and(|l| (l.bottom - bottom).abs() < (size.max(l.size) * 0.5).max(1.0));
            if !same_line {
                flush(current.take(), &mut sizes, &mut out);
                current = Some(Line {
                    page: index,
                    text: String::new(),
                    size,
                    top,
                    bottom,
                });
            }
            if let Some(l) = current.as_mut() {
                l.text.push_str(&s);
                l.top = l.top.min(top);
                l.bottom = l.bottom.max(bottom);
                if !s.trim().is_empty() {
                    sizes.push(size);
                }
            }
        }
        flush(current.take(), &mut sizes, &mut out);
    }
    if out.is_empty() {
        return Err(no_text());
    }
    Ok(out)
}

/// Rebuild Markdown from the lines the pages show.
pub fn markdown(lines: &[Line]) -> String {
    let pages = lines.iter().map(|l| l.page).max().map_or(1, |p| p + 1);
    // Running headers and footers: the same text near the top or bottom of at
    // least half the pages (with page numbers counted as one text).
    let mut seen: HashMap<String, usize> = HashMap::new();
    let key = |t: &str| {
        t.chars()
            .map(|c| if c.is_ascii_digit() { '#' } else { c })
            .collect::<String>()
    };
    for l in lines {
        *seen.entry(key(&l.text)).or_default() += 1;
    }
    let repeated =
        |l: &Line| pages >= 3 && seen.get(&key(&l.text)).is_some_and(|n| *n * 2 >= pages);
    let page_number = |t: &str| {
        t.chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace() || "-–/.".contains(c))
            && t.chars().any(|c| c.is_ascii_digit())
    };
    let kept: Vec<&Line> = lines
        .iter()
        .filter(|l| !repeated(l) && !page_number(&l.text))
        .collect();

    // The body's size: the one most characters are set in.
    let mut weight: HashMap<u32, usize> = HashMap::new();
    for l in &kept {
        *weight.entry((l.size * 2.0).round() as u32).or_default() += l.text.chars().count();
    }
    let body = weight
        .iter()
        .max_by_key(|(_, n)| **n)
        .map_or(10.0, |(s, _)| *s as f32 / 2.0);
    let mut heading_sizes: Vec<u32> = weight
        .keys()
        .copied()
        .filter(|s| *s as f32 / 2.0 >= body * 1.15)
        .collect();
    heading_sizes.sort_unstable_by(|a, b| b.cmp(a));
    let level = |size: f32| {
        let s = (size * 2.0).round() as u32;
        heading_sizes
            .iter()
            .position(|h| *h == s)
            .map(|i| (i + 1).min(4))
    };

    let mut out = String::new();
    let mut para = String::new();
    let mut prev: Option<&Line> = None;
    let flush = |out: &mut String, para: &mut String| {
        if !para.trim().is_empty() {
            out.push_str(para.trim());
            out.push_str("\n\n");
        }
        para.clear();
    };
    for l in kept {
        let text = l.text.as_str();
        if let Some(h) = level(l.size).filter(|_| text.chars().count() < 200) {
            flush(&mut out, &mut para);
            let _ = write!(out, "{} {}\n\n", "#".repeat(h), escape(text));
            prev = Some(l);
            continue;
        }
        let bullet = ["•", "◦", "▪", "‣", "●", "○", "■", "– ", "- ", "* "]
            .iter()
            .find_map(|b| text.strip_prefix(b))
            .map(str::trim_start);
        let numbered = numbered_item(text);
        let gap = prev.is_some_and(|p| {
            p.page != l.page
                || l.top - p.bottom > l.size.max(p.size) * 0.9
                || (p.size - l.size).abs() > 0.6
        });
        if let Some(item) = bullet {
            flush(&mut out, &mut para);
            para.push_str("- ");
            para.push_str(&escape(item));
        } else if let Some((n, item)) = numbered {
            flush(&mut out, &mut para);
            let _ = write!(para, "{n}. {}", escape(item));
        } else if gap && !(para.starts_with("- ") && prev.is_some_and(|p| p.page == l.page))
            || para.is_empty()
        {
            flush(&mut out, &mut para);
            para.push_str(&escape(text));
        } else if para.ends_with('-') && text.chars().next().is_some_and(char::is_lowercase) {
            // A word split at the line's end.
            para.pop();
            para.push_str(&escape(text));
        } else {
            para.push(' ');
            para.push_str(&escape(text));
        }
        prev = Some(l);
    }
    flush(&mut out, &mut para);
    out
}

fn numbered_item(text: &str) -> Option<(&str, &str)> {
    let digits = text.find(|c: char| !c.is_ascii_digit())?;
    if digits == 0 || digits > 3 {
        return None;
    }
    let rest = &text[digits..];
    let item = rest
        .strip_prefix(". ")
        .or_else(|| rest.strip_prefix(") "))?;
    Some((&text[..digits], item.trim_start()))
}

/// Text from a page that Markdown would otherwise read as markup.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\`*_[]<>|".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    if out.starts_with('#') || out.starts_with('>') || out.starts_with('+') {
        out.insert(0, '\\');
    }
    out
}

/// Every page as a picture, at 150 dots per inch. One page gives one file;
/// more give a folder named after the file, with one picture per page.
fn pages_as_images(
    doc: &PdfDocument<'_>,
    to: &str,
    output: &Path,
    quality: u8,
) -> Result<(), Error> {
    let count = doc.pages().len() as usize;
    let folder: PathBuf = if count > 1 {
        let f = output.with_extension("");
        std::fs::create_dir_all(&f).map_err(|e| Error::Io(e.to_string()))?;
        f
    } else {
        PathBuf::new()
    };
    let stem = output
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    for (i, page) in doc.pages().iter().enumerate() {
        let width = (page.width().value / 72.0 * 150.0).round() as Pixels;
        let bitmap = page
            .render_with_config(&PdfRenderConfig::new().set_target_width(width.max(1)))
            .map_err(|e| Error::Engine(e.to_string()))?;
        let image = bitmap
            .as_image()
            .map_err(|e| Error::Engine(e.to_string()))?;
        let path = if count > 1 {
            folder.join(format!("{stem}-{:03}.{to}", i + 1))
        } else {
            output.to_path_buf()
        };
        let file = std::fs::File::create(&path).map_err(|e| Error::Io(e.to_string()))?;
        let mut w = std::io::BufWriter::new(file);
        if to == "jpg" {
            image
                .to_rgb8()
                .write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
                    &mut w, quality,
                ))
        } else {
            image.write_to(&mut w, image::ImageFormat::Png)
        }
        .map_err(|e| Error::Engine(e.to_string()))?;
    }
    // With several pages the result is the folder (`output` without its
    // extension); the converter looks for it there.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(page: usize, text: &str, size: f32, top: f32) -> Line {
        Line {
            page,
            text: text.into(),
            size,
            top,
            bottom: top + size,
        }
    }

    #[test]
    fn markdown_is_rebuilt_from_what_the_page_shows() {
        let lines = vec![
            line(0, "Annual Report", 24.0, 50.0),
            line(0, "Introduction", 16.0, 100.0),
            line(0, "This is the first para-", 10.0, 130.0),
            line(0, "graph of the report.", 10.0, 142.0),
            line(0, "A second paragraph.", 10.0, 170.0),
            line(0, "• first point", 10.0, 200.0),
            line(0, "• second point", 10.0, 212.0),
            line(0, "2. numbered", 10.0, 240.0),
            line(0, "1", 10.0, 780.0),
        ];
        let md = markdown(&lines);
        assert!(
            md.starts_with("# Annual Report\n\n## Introduction\n\n"),
            "{md}"
        );
        assert!(
            md.contains("This is the first paragraph of the report.\n\n"),
            "{md}"
        );
        assert!(
            md.contains("A second paragraph.\n\n- first point\n\n- second point\n\n2. numbered"),
            "{md}"
        );
        assert!(!md.contains("\n1\n"), "page numbers are left out: {md}");
    }
}
