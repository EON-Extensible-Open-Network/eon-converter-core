// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Conversions the core does itself, with nothing to fetch: data formats,
//! spreadsheets, plain text to Markdown, common images, archives.

use super::{pairs, Edge, Step};
use crate::format::Format;
use crate::tools::Engine;
use crate::Error;
use serde_json::{Map, Value};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::Path;

const DATA: &[&str] = &["json", "yaml", "toml"];
const TABLE_TEXT: &[&str] = &["csv", "tsv"];
const SHEETS: &[&str] = &["xlsx", "xls", "xlsb", "ods"];
const IMAGE_IN: &[&str] = &[
    "png", "jpg", "gif", "bmp", "ico", "tiff", "webp", "tga", "qoi", "pnm", "hdr", "dds",
];
const IMAGE_OUT: &[&str] = &[
    "png", "jpg", "gif", "bmp", "ico", "tiff", "webp", "tga", "qoi", "pnm",
];
const ARCHIVES: &[&str] = &["zip", "tar", "tgz"];

pub fn edges(all: &mut Vec<Edge>) {
    let e = Engine::Core;
    pairs(all, e, DATA, DATA, 1, |_, _| false);
    pairs(all, e, DATA, &["xml"], 2, |_, _| true);
    pairs(all, e, &["xml"], DATA, 2, |_, _| true);
    pairs(all, e, TABLE_TEXT, TABLE_TEXT, 1, |_, _| false);
    pairs(all, e, TABLE_TEXT, &["json", "yaml"], 2, |_, _| false);
    pairs(all, e, &["json", "yaml"], TABLE_TEXT, 2, |_, _| true);
    // A spreadsheet's values, without its formatting or formulas.
    pairs(
        all,
        e,
        SHEETS,
        &["csv", "tsv", "json", "html", "md"],
        2,
        |_, _| true,
    );
    pairs(all, e, SHEETS, &["xlsx"], 3, |_, _| true);
    pairs(all, e, &["csv", "tsv", "json"], &["xlsx"], 2, |_, _| false);
    pairs(all, e, &["txt"], &["md"], 1, |_, _| false);
    // Images. GIF and WebP may be animated; between them ImageMagick keeps the
    // animation, so the core does not offer that pair.
    for from in IMAGE_IN {
        for to in IMAGE_OUT {
            let animated_pair = matches!((*from, *to), ("gif", "webp") | ("webp", "gif"));
            if from != to && !animated_pair {
                let lossy = *to == "jpg"
                    || *to == "gif"
                    || *to == "ico"
                    || ["gif", "webp", "hdr", "tiff"].contains(from);
                all.push(Edge {
                    from,
                    to,
                    engine: e,
                    cost: 2,
                    lossy,
                });
            }
        }
    }
    pairs(all, e, ARCHIVES, ARCHIVES, 1, |_, _| false);
}

pub async fn convert(
    step: &mut Step<'_>,
    from: &Format,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    let (from, to) = (from.id, to.id);
    let (input, output) = (input.to_path_buf(), output.to_path_buf());
    let quality = step.options.jpeg_quality();
    (step.report)(0.0);
    tokio::task::spawn_blocking(move || blocking(from, to, &input, &output, quality))
        .await
        .map_err(|e| Error::Io(e.to_string()))?
}

fn blocking(from: &str, to: &str, input: &Path, output: &Path, quality: u8) -> Result<(), Error> {
    if ARCHIVES.contains(&from) {
        return archive(from, to, input, output);
    }
    if IMAGE_IN.contains(&from) && IMAGE_OUT.contains(&to) {
        return image(to, input, output, quality);
    }
    if from == "txt" {
        let text = read_text(input)?;
        return write(output, text_to_markdown(&text).as_bytes());
    }
    if SHEETS.contains(&from) {
        let book = read_workbook(input)?;
        return match to {
            "xlsx" => write_xlsx(&book, output),
            _ => write(output, &book_as(&book, to)?),
        };
    }
    let value = read_value(from, input)?;
    if to == "xlsx" {
        let rows = value_to_rows(&value);
        return write_xlsx(&[("Sheet1".to_owned(), rows)], output);
    }
    write(output, &write_value(to, &value)?)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    std::fs::write(path, bytes).map_err(|e| Error::Io(e.to_string()))
}

fn bad(what: impl std::fmt::Display) -> Error {
    Error::Invalid(what.to_string())
}

// ---- text ------------------------------------------------------------------------

/// A text file as text: UTF-8 or UTF-16 (by its BOM); otherwise the legacy code
/// page most such files come from on a Turkish system (Windows-1254).
pub fn read_text(path: &Path) -> Result<String, Error> {
    let bytes = std::fs::read(path).map_err(|e| Error::Io(e.to_string()))?;
    if let Some((encoding, _)) = encoding_rs::Encoding::for_bom(&bytes) {
        return Ok(encoding.decode(&bytes).0.into_owned());
    }
    match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(e) => Ok(encoding_rs::WINDOWS_1254
            .decode(e.as_bytes())
            .0
            .into_owned()),
    }
}

/// Plain text as Markdown that reads back as exactly the same text: every
/// character Markdown would interpret is escaped, and every line break is kept.
pub fn text_to_markdown(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            out.push('\n');
            continue;
        }
        // Leading spaces would make a code block: keep them as non-breaking.
        let indent = trimmed.len() - trimmed.trim_start().len();
        out.push_str(&"\u{a0}".repeat(indent));
        for c in trimmed.trim_start().chars() {
            if "\\`*_{}[]<>()#+-.!|~$&".contains(c) {
                out.push('\\');
            }
            out.push(c);
        }
        let next_is_text = lines.get(i + 1).is_some_and(|n| !n.trim().is_empty());
        out.push_str(if next_is_text { "\\\n" } else { "\n" });
    }
    out
}

// ---- data ------------------------------------------------------------------------

fn read_value(from: &str, path: &Path) -> Result<Value, Error> {
    let text = read_text(path)?;
    match from {
        "json" => serde_json::from_str(&text).map_err(bad),
        "yaml" => serde_yaml_ng::from_str(&text).map_err(bad),
        "toml" => {
            let v: toml::Value = toml::from_str(&text).map_err(bad)?;
            serde_json::to_value(v).map_err(bad)
        }
        "xml" => xml_to_value(&text),
        "csv" | "tsv" => table_to_value(&text, if from == "csv" { b',' } else { b'\t' }),
        other => Err(bad(format!("the core does not read {other}"))),
    }
}

fn write_value(to: &str, value: &Value) -> Result<Vec<u8>, Error> {
    Ok(match to {
        "json" => serde_json::to_vec_pretty(value).map_err(bad)?,
        "yaml" => serde_yaml_ng::to_string(value).map_err(bad)?.into_bytes(),
        "toml" => {
            // TOML has no null and needs a table at the top.
            let v = without_nulls(value.clone());
            let table = if v.is_object() {
                v
            } else {
                Value::Object(Map::from_iter([("items".to_owned(), v)]))
            };
            toml::to_string_pretty(&table).map_err(bad)?.into_bytes()
        }
        "xml" => value_to_xml(value).into_bytes(),
        "csv" | "tsv" => rows_to_table(
            &value_to_rows(value),
            if to == "csv" { b',' } else { b'\t' },
        )?,
        other => return Err(bad(format!("the core does not write {other}"))),
    })
}

fn without_nulls(v: Value) -> Value {
    match v {
        Value::Object(m) => Value::Object(
            m.into_iter()
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k, without_nulls(v)))
                .collect(),
        ),
        Value::Array(a) => Value::Array(
            a.into_iter()
                .filter(|v| !v.is_null())
                .map(without_nulls)
                .collect(),
        ),
        other => other,
    }
}

fn table_to_value(text: &str, delimiter: u8) -> Result<Value, Error> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_reader(text.as_bytes());
    let headers: Vec<String> = reader
        .headers()
        .map_err(bad)?
        .iter()
        .map(str::to_owned)
        .collect();
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(bad)?;
        let mut row = Map::new();
        for (i, cell) in record.iter().enumerate() {
            let key = headers
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("column{}", i + 1));
            row.insert(key, Value::String(cell.to_owned()));
        }
        rows.push(Value::Object(row));
    }
    Ok(Value::Array(rows))
}

/// Rows of cells for a table: an array of objects gives a header row of every key;
/// an array of arrays is used as it is; anything else is one cell.
fn value_to_rows(value: &Value) -> Vec<Vec<String>> {
    let cell = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    };
    match value {
        Value::Array(items) if items.iter().all(Value::is_object) => {
            let mut keys: Vec<String> = Vec::new();
            for item in items {
                if let Value::Object(m) = item {
                    for k in m.keys() {
                        if !keys.contains(k) {
                            keys.push(k.clone());
                        }
                    }
                }
            }
            let mut rows = vec![keys.clone()];
            for item in items {
                rows.push(
                    keys.iter()
                        .map(|k| item.get(k).map(cell).unwrap_or_default())
                        .collect(),
                );
            }
            rows
        }
        Value::Array(items) => items
            .iter()
            .map(|r| match r {
                Value::Array(cells) => cells.iter().map(cell).collect(),
                other => vec![cell(other)],
            })
            .collect(),
        Value::Object(m) => {
            let mut rows = vec![vec!["key".to_owned(), "value".to_owned()]];
            rows.extend(m.iter().map(|(k, v)| vec![k.clone(), cell(v)]));
            rows
        }
        other => vec![vec![cell(other)]],
    }
}

fn rows_to_table(rows: &[Vec<String>], delimiter: u8) -> Result<Vec<u8>, Error> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .from_writer(Vec::new());
    for row in rows {
        writer.write_record(row).map_err(bad)?;
    }
    writer.into_inner().map_err(bad)
}

/// XML as data: an element is an object; attributes are `@name`, text is
/// `#text`, and repeated children become an array.
fn xml_to_value(text: &str) -> Result<Value, Error> {
    use quick_xml::events::{BytesStart, Event};
    use quick_xml::XmlVersion;
    fn attributes(e: &BytesStart<'_>) -> Result<Map<String, Value>, Error> {
        let mut m = Map::new();
        for a in e.attributes().flatten() {
            let key = format!("@{}", a.key.as_ref());
            let value = a.normalized_value(XmlVersion::Implicit1_0).map_err(bad)?;
            m.insert(key, Value::String(value.into_owned()));
        }
        Ok(m)
    }
    fn flush_text(stack: &mut [(String, Map<String, Value>, String)]) {
        if let Some((_, m, text)) = stack.last_mut() {
            let t = text.trim();
            if !t.is_empty() {
                m.insert("#text".into(), Value::String(t.to_owned()));
            }
            text.clear();
        }
    }
    let mut reader = quick_xml::Reader::from_str(text);
    let mut stack: Vec<(String, Map<String, Value>, String)> =
        vec![("#document".into(), Map::new(), String::new())];
    loop {
        match reader.read_event().map_err(bad)? {
            Event::Start(e) => {
                flush_text(&mut stack);
                stack.push((e.name().as_ref().to_owned(), attributes(&e)?, String::new()));
            }
            Event::Empty(e) => {
                flush_text(&mut stack);
                let m = attributes(&e)?;
                let name = e.name().as_ref().to_owned();
                if let Some((_, parent, _)) = stack.last_mut() {
                    add_child(parent, name, Value::Object(m));
                }
            }
            Event::Text(t) => {
                if let Some((_, _, buffer)) = stack.last_mut() {
                    buffer.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                let resolved = match r.resolve_char_ref().map_err(bad)? {
                    Some(c) => c.to_string(),
                    None => match r.as_ref() {
                        "amp" => "&".into(),
                        "lt" => "<".into(),
                        "gt" => ">".into(),
                        "quot" => "\"".into(),
                        "apos" => "'".into(),
                        other => format!("&{other};"),
                    },
                };
                if let Some((_, _, buffer)) = stack.last_mut() {
                    buffer.push_str(&resolved);
                }
            }
            Event::CData(t) => {
                if let Some((_, _, buffer)) = stack.last_mut() {
                    buffer.push_str(t.as_ref());
                }
            }
            Event::End(_) => {
                flush_text(&mut stack);
                let (name, m, _) = stack.pop().ok_or_else(|| bad("unbalanced XML"))?;
                let value = if m.len() == 1 && m.contains_key("#text") {
                    m["#text"].clone()
                } else {
                    Value::Object(m)
                };
                if let Some((_, parent, _)) = stack.last_mut() {
                    add_child(parent, name, value);
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let (_, root, _) = stack.pop().ok_or_else(|| bad("empty XML"))?;
    Ok(Value::Object(root))
}

fn add_child(parent: &mut Map<String, Value>, name: String, value: Value) {
    match parent.get_mut(&name) {
        Some(Value::Array(a)) => a.push(value),
        Some(existing) => {
            let first = existing.take();
            *existing = Value::Array(vec![first, value]);
        }
        None => {
            parent.insert(name, value);
        }
    }
}

fn value_to_xml(value: &Value) -> String {
    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    fn name(s: &str) -> String {
        let n: String = s
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || "_-.".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        if n.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_')
        {
            n
        } else {
            format!("_{n}")
        }
    }
    fn element(out: &mut String, tag: &str, value: &Value, depth: usize) {
        let pad = "  ".repeat(depth);
        match value {
            Value::Array(items) => {
                for item in items {
                    element(out, tag, item, depth);
                }
            }
            Value::Object(m) => {
                let attrs: String = m
                    .iter()
                    .filter_map(|(k, v)| {
                        k.strip_prefix('@').map(|a| {
                            format!(
                                " {}=\"{}\"",
                                name(a),
                                escape(&v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                            )
                        })
                    })
                    .collect();
                let children: Vec<_> = m.iter().filter(|(k, _)| !k.starts_with('@')).collect();
                if children.is_empty() {
                    let _ = writeln!(out, "{pad}<{tag}{attrs}/>");
                    return;
                }
                let _ = writeln!(out, "{pad}<{tag}{attrs}>");
                for (k, v) in children {
                    if k == "#text" {
                        let _ = writeln!(
                            out,
                            "{pad}  {}",
                            escape(&v.as_str().map_or_else(|| v.to_string(), str::to_owned))
                        );
                    } else {
                        element(out, &name(k), v, depth + 1);
                    }
                }
                let _ = writeln!(out, "{pad}</{tag}>");
            }
            Value::Null => {
                let _ = writeln!(out, "{pad}<{tag}/>");
            }
            Value::String(s) => {
                let _ = writeln!(out, "{pad}<{tag}>{}</{tag}>", escape(s));
            }
            other => {
                let _ = writeln!(out, "{pad}<{tag}>{other}</{tag}>");
            }
        }
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    match value {
        Value::Object(m) if m.len() == 1 && !m.values().any(Value::is_array) => {
            let (k, v) = m
                .iter()
                .next()
                .map(|(k, v)| (name(k), v))
                .unwrap_or_default();
            element(&mut out, &k, v, 0);
        }
        Value::Array(_) => {
            out.push_str("<root>\n");
            element(&mut out, "item", value, 1);
            out.push_str("</root>\n");
        }
        other => element(&mut out, "root", other, 0),
    }
    out
}

// ---- spreadsheets -------------------------------------------------------------------

type Book = Vec<(String, Vec<Vec<String>>)>;

fn read_workbook(path: &Path) -> Result<Book, Error> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut wb = open_workbook_auto(path).map_err(bad)?;
    let mut book = Vec::new();
    for name in wb.sheet_names() {
        let range = wb.worksheet_range(&name).map_err(bad)?;
        let rows = range
            .rows()
            .map(|r| {
                r.iter()
                    .map(|c| match c {
                        Data::Empty => String::new(),
                        Data::String(s) | Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
                        Data::Float(f) => {
                            if f.fract() == 0.0 && f.abs() < 1e15 {
                                format!("{}", *f as i64)
                            } else {
                                f.to_string()
                            }
                        }
                        Data::Int(i) => i.to_string(),
                        Data::Bool(b) => b.to_string(),
                        Data::DateTime(d) => d.as_datetime().map_or_else(
                            || d.to_string(),
                            |t| {
                                if t.time() == chrono_midnight() {
                                    t.date().to_string()
                                } else {
                                    t.to_string()
                                }
                            },
                        ),
                        Data::Error(e) => format!("#{e:?}"),
                    })
                    .collect()
            })
            .collect();
        book.push((name, rows));
    }
    Ok(book)
}

fn book_as(book: &Book, to: &str) -> Result<Vec<u8>, Error> {
    let first = book
        .first()
        .map(|(_, rows)| rows.clone())
        .unwrap_or_default();
    match to {
        "csv" => rows_to_table(&first, b','),
        "tsv" => rows_to_table(&first, b'\t'),
        "json" => {
            let sheet = |rows: &Vec<Vec<String>>| {
                let mut it = rows.iter();
                let header: Vec<String> = it.next().cloned().unwrap_or_default();
                Value::Array(
                    it.map(|r| {
                        Value::Object(
                            r.iter()
                                .enumerate()
                                .map(|(i, c)| {
                                    (
                                        header
                                            .get(i)
                                            .filter(|h| !h.is_empty())
                                            .cloned()
                                            .unwrap_or_else(|| format!("column{}", i + 1)),
                                        Value::String(c.clone()),
                                    )
                                })
                                .collect(),
                        )
                    })
                    .collect(),
                )
            };
            let v = if book.len() == 1 {
                sheet(&first)
            } else {
                Value::Object(book.iter().map(|(n, r)| (n.clone(), sheet(r))).collect())
            };
            serde_json::to_vec_pretty(&v).map_err(bad)
        }
        "html" => {
            let esc = |s: &str| {
                s.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            };
            let mut out = String::from("<!doctype html>\n<meta charset=\"utf-8\">\n<style>table{border-collapse:collapse}td,th{border:1px solid #999;padding:4px 8px}</style>\n");
            for (name, rows) in book {
                let _ = writeln!(out, "<h2>{}</h2>\n<table>", esc(name));
                for (i, row) in rows.iter().enumerate() {
                    let tag = if i == 0 { "th" } else { "td" };
                    out.push_str("<tr>");
                    for c in row {
                        let _ = write!(out, "<{tag}>{}</{tag}>", esc(c));
                    }
                    out.push_str("</tr>\n");
                }
                out.push_str("</table>\n");
            }
            Ok(out.into_bytes())
        }
        "md" => {
            let esc = |s: &str| s.replace('|', "\\|").replace('\n', " ");
            let mut out = String::new();
            for (name, rows) in book {
                let width = rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
                let _ = writeln!(out, "## {}\n", esc(name));
                for (i, row) in rows.iter().enumerate() {
                    let cells: Vec<String> = (0..width)
                        .map(|j| esc(row.get(j).map_or("", String::as_str)))
                        .collect();
                    let _ = writeln!(out, "| {} |", cells.join(" | "));
                    if i == 0 {
                        let _ = writeln!(out, "|{}", " --- |".repeat(width));
                    }
                }
                out.push('\n');
            }
            Ok(out.into_bytes())
        }
        other => Err(bad(format!("a spreadsheet does not become {other} here"))),
    }
}

fn chrono_midnight() -> chrono::NaiveTime {
    chrono::NaiveTime::MIN
}

fn write_xlsx(book: &[(String, Vec<Vec<String>>)], path: &Path) -> Result<(), Error> {
    let mut wb = rust_xlsxwriter::Workbook::new();
    for (name, rows) in book {
        let sheet = wb.add_worksheet();
        let safe: String = name
            .chars()
            .filter(|c| !"[]:*?/\\".contains(*c))
            .take(31)
            .collect();
        if !safe.is_empty() {
            sheet.set_name(&safe).map_err(bad)?;
        }
        for (r, row) in rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                let (r, c) = (r as u32, c as u16);
                match cell.parse::<f64>() {
                    Ok(n) if !cell.is_empty() && !cell.starts_with('0') || cell == "0" => {
                        sheet.write_number(r, c, n).map_err(bad)?;
                    }
                    _ => {
                        sheet.write_string(r, c, cell).map_err(bad)?;
                    }
                }
            }
        }
    }
    wb.save(path).map_err(bad)
}

// ---- images -------------------------------------------------------------------------

fn image(to: &str, input: &Path, output: &Path, quality: u8) -> Result<(), Error> {
    use image::{codecs, ImageReader};
    let img = ImageReader::open(input)
        .map_err(|e| Error::Io(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| Error::Io(e.to_string()))?
        .decode()
        .map_err(bad)?;
    let file = std::fs::File::create(output).map_err(|e| Error::Io(e.to_string()))?;
    let mut w = std::io::BufWriter::new(file);
    match to {
        "jpg" => img
            .to_rgb8()
            .write_with_encoder(codecs::jpeg::JpegEncoder::new_with_quality(&mut w, quality)),
        "png" => img.write_to(&mut w, image::ImageFormat::Png),
        "gif" => img.to_rgba8().write_to(&mut w, image::ImageFormat::Gif),
        "bmp" => img.to_rgba8().write_to(&mut w, image::ImageFormat::Bmp),
        "ico" => img
            .thumbnail(256, 256)
            .to_rgba8()
            .write_to(&mut w, image::ImageFormat::Ico),
        "tiff" => img.to_rgba8().write_to(&mut w, image::ImageFormat::Tiff),
        "webp" => img
            .to_rgba8()
            .write_with_encoder(codecs::webp::WebPEncoder::new_lossless(&mut w)),
        "tga" => img.to_rgba8().write_to(&mut w, image::ImageFormat::Tga),
        "qoi" => img.to_rgba8().write_to(&mut w, image::ImageFormat::Qoi),
        "pnm" => img.to_rgb8().write_to(&mut w, image::ImageFormat::Pnm),
        other => return Err(bad(format!("the core does not write {other} images"))),
    }
    .map_err(bad)?;
    w.flush().map_err(|e| Error::Io(e.to_string()))
}

// ---- archives -----------------------------------------------------------------------

/// One file of an archive, read into memory one at a time.
struct Entry {
    name: String,
    data: Vec<u8>,
    dir: bool,
}

enum Out {
    Zip(Box<zip::ZipWriter<std::fs::File>>),
    Tar(Box<tar::Builder<std::fs::File>>),
    Tgz(Box<tar::Builder<flate2::write::GzEncoder<std::fs::File>>>),
}

impl Out {
    fn add(&mut self, e: &Entry) -> Result<(), Error> {
        match self {
            Out::Zip(z) => {
                let opts = zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated);
                if e.dir {
                    z.add_directory(e.name.trim_end_matches('/'), opts)
                        .map_err(bad)
                } else {
                    z.start_file(e.name.as_str(), opts).map_err(bad)?;
                    z.write_all(&e.data).map_err(|e| Error::Io(e.to_string()))
                }
            }
            Out::Tar(t) => tar_add(t, e),
            Out::Tgz(t) => tar_add(t, e),
        }
    }

    fn finish(self) -> Result<(), Error> {
        match self {
            Out::Zip(z) => z.finish().map(|_| ()).map_err(bad),
            Out::Tar(t) => t
                .into_inner()
                .map(|_| ())
                .map_err(|e| Error::Io(e.to_string())),
            Out::Tgz(t) => t
                .into_inner()
                .and_then(flate2::write::GzEncoder::finish)
                .map(|_| ())
                .map_err(|e| Error::Io(e.to_string())),
        }
    }
}

fn archive(from: &str, to: &str, input: &Path, output: &Path) -> Result<(), Error> {
    let file = std::fs::File::open(input).map_err(|e| Error::Io(e.to_string()))?;
    let created = std::fs::File::create(output).map_err(|e| Error::Io(e.to_string()))?;
    let mut out = match to {
        "zip" => Out::Zip(Box::new(zip::ZipWriter::new(created))),
        "tar" => Out::Tar(Box::new(tar::Builder::new(created))),
        _ => Out::Tgz(Box::new(tar::Builder::new(flate2::write::GzEncoder::new(
            created,
            flate2::Compression::default(),
        )))),
    };
    if from == "zip" {
        let mut zip = zip::ZipArchive::new(file).map_err(bad)?;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(bad)?;
            let name = f
                .enclosed_name()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .ok_or_else(|| bad("an unsafe path in the archive"))?;
            let mut data = Vec::new();
            f.read_to_end(&mut data)
                .map_err(|e| Error::Io(e.to_string()))?;
            out.add(&Entry {
                name,
                data,
                dir: f.is_dir(),
            })?;
        }
    } else {
        let reader: Box<dyn Read> = if from == "tgz" {
            Box::new(flate2::read::GzDecoder::new(file))
        } else {
            Box::new(file)
        };
        let mut tar = tar::Archive::new(reader);
        for entry in tar.entries().map_err(bad)? {
            let mut entry = entry.map_err(bad)?;
            let path = entry.path().map_err(bad)?.into_owned();
            if path.components().any(|c| {
                matches!(
                    c,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                return Err(bad("an unsafe path in the archive"));
            }
            let dir = entry.header().entry_type().is_dir();
            let mut data = Vec::new();
            entry
                .read_to_end(&mut data)
                .map_err(|e| Error::Io(e.to_string()))?;
            let name = path.to_string_lossy().replace('\\', "/");
            out.add(&Entry { name, data, dir })?;
        }
    }
    out.finish()
}

fn tar_add<W: Write>(tar: &mut tar::Builder<W>, e: &Entry) -> Result<(), Error> {
    let mut header = tar::Header::new_gnu();
    header.set_size(if e.dir { 0 } else { e.data.len() as u64 });
    header.set_mode(if e.dir { 0o755 } else { 0o644 });
    header.set_entry_type(if e.dir {
        tar::EntryType::Directory
    } else {
        tar::EntryType::Regular
    });
    header.set_cksum();
    tar.append_data(&mut header, &e.name, e.data.as_slice())
        .map_err(|e| Error::Io(e.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_survives_markdown() {
        let md = text_to_markdown("# not a heading\n1. not a list\nline two\n\n  indented *stars*");
        assert!(md.starts_with("\\# not a heading\\\n1\\. not a list\\\nline two\n\n"));
        assert!(md.contains("\\*stars\\*"));
    }

    #[test]
    fn data_goes_round() {
        let v: Value = serde_json::json!({"name": "EON", "tags": ["a", "b"], "n": 3});
        for to in ["json", "yaml", "toml"] {
            let bytes = write_value(to, &v).unwrap();
            let dir = std::env::temp_dir().join(format!("eon-conv-data-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let p = dir.join(format!("x.{to}"));
            std::fs::write(&p, bytes).unwrap();
            assert_eq!(read_value(to, &p).unwrap(), v, "{to}");
        }
        let xml = value_to_xml(
            &serde_json::json!({"book": {"@id": "1", "title": "T", "tag": ["a", "b"]}}),
        );
        let back = xml_to_value(&xml).unwrap();
        assert_eq!(back["book"]["@id"], "1");
        assert_eq!(back["book"]["tag"][1], "b");
        let rows = value_to_rows(&serde_json::json!([{"a": 1, "b": "x"}, {"a": 2}]));
        assert_eq!(rows, vec![vec!["a", "b"], vec!["1", "x"], vec!["2", ""]]);
    }
}
