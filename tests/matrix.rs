// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Real files through real engines. Each family's files are made here, converted
//! with the pinned engines (fetched the first time into
//! `target/converter-test-tools`), and each result is checked: what it is by its
//! content, and for media the streams it holds. The owner's own examples (plan
//! madde 48) are among them by name.
//!
//! ```text
//! cargo test --test matrix -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use eon_converter_core::{format, tools::Engine, Converter, Options};
use std::path::{Path, PathBuf};
use tokio::sync::watch;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("target/converter-test-tools")
}

fn out_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("eon-converter-matrix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Run {
    conv: Converter,
    rt: tokio::runtime::Runtime,
    dir: PathBuf,
    failures: Vec<String>,
    passed: usize,
}

impl Run {
    fn convert(&self, input: &Path, to: &str) -> Result<PathBuf, String> {
        let (_tx, rx) = watch::channel(false);
        let folder = self.dir.join("out");
        self.rt
            .block_on(
                self.conv
                    .convert(input, to, Some(&folder), &Options::default(), &|_| {}, rx),
            )
            .map_err(|e| e.to_string())
    }

    /// Convert and check the result is what was asked for.
    fn check(&mut self, input: &Path, to: &str) -> Option<PathBuf> {
        let label = format!("{} -> {to}", input.file_name().unwrap().to_string_lossy());
        match self.convert(input, to) {
            Ok(out) if out.is_dir() => {
                println!("ok    {label} (folder of pages)");
                self.passed += 1;
                Some(out)
            }
            Ok(out) => {
                let size = std::fs::metadata(&out).map_or(0, |m| m.len());
                let got = format::detect_file(&out)
                    .ok()
                    .flatten()
                    .map_or("?", |f| f.id);
                let want = format::by_id(to).unwrap();
                // A text format may be told only by its name; a container by its content.
                let same = format::by_id(got).is_some_and(|g| format::agrees(g, want));
                if size == 0 || !same {
                    self.failures
                        .push(format!("{label}: made {got}, {size} bytes"));
                    println!("FAIL  {label}: made {got}, {size} bytes");
                    return None;
                }
                println!("ok    {label} ({size} bytes)");
                self.passed += 1;
                Some(out)
            }
            Err(e) => {
                println!("FAIL  {label}: {e}");
                self.failures.push(format!("{label}: {e}"));
                None
            }
        }
    }

    fn streams(&self, file: &Path) -> Vec<String> {
        let ffmpeg = self.rt.block_on(self.conv.fetch(Engine::Ffmpeg)).unwrap();
        let out = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-i"])
            .arg(file)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stderr)
            .lines()
            .filter(|l| l.trim_start().starts_with("Stream #"))
            .filter_map(|l| {
                l.split_once(": ")
                    .map(|(_, r)| r.split(':').next().unwrap_or_default().to_owned())
            })
            .collect()
    }

    fn expect_streams(&mut self, file: Option<&PathBuf>, want: &[&str]) {
        let Some(file) = file else { return };
        let got = self.streams(file);
        if got != want {
            self.failures.push(format!(
                "{}: streams {got:?}, wanted {want:?}",
                file.display()
            ));
            println!("FAIL  {} streams {got:?}", file.display());
        }
    }

    fn expect_text(&mut self, file: Option<&PathBuf>, needles: &[&str]) {
        let Some(file) = file else { return };
        let text = std::fs::read_to_string(file).unwrap_or_default();
        for n in needles {
            if !text.contains(n) {
                self.failures
                    .push(format!("{}: does not contain {n:?}", file.display()));
                println!("FAIL  {} does not contain {n:?}", file.display());
            }
        }
    }
}

const TEXT: &str = "EON Converter\n\nÇok güzel bir gün: ğüşıöç ĞÜŞİÖÇ.\n# not a heading\n1. not a list\n\nSecond paragraph, with *stars* and _underscores_.\n";

const MARKDOWN: &str = "# Başlık\n\nBir paragraf, **kalın** ve *eğik*.\n\n## İkinci bölüm\n\n- birinci madde\n- ikinci madde\n\n| ad | sayı |\n|----|------|\n| elma | 3 |\n| armut | 5 |\n";

#[test]
#[ignore = "network: fetches the engines (about 280 MB the first time)"]
#[allow(clippy::too_many_lines)] // one matrix, read top to bottom
fn every_family_converts_for_real() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let conv = {
        let _g = rt.enter();
        Converter::new(&data_dir()).unwrap()
    };
    let dir = out_dir();
    let mut r = Run {
        conv,
        rt,
        dir: dir.clone(),
        failures: Vec::new(),
        passed: 0,
    };
    let src = dir.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let write = |name: &str, body: &[u8]| {
        let p = src.join(name);
        std::fs::write(&p, body).unwrap();
        p
    };

    // ---- the owner's examples ----------------------------------------------------
    let txt = write("notlar ğüş.txt", TEXT.as_bytes());
    let pdf = r.check(&txt, "pdf");
    if let Some(pdf) = &pdf {
        let md = r.check(pdf, "md");
        r.expect_text(md.as_ref(), &["ğüşıöç", "Second paragraph"]);
        r.check(pdf, "txt");
        r.check(pdf, "png");
        r.check(pdf, "docx");
    }

    // ---- documents -------------------------------------------------------------------
    let md = write("belge.md", MARKDOWN.as_bytes());
    for to in [
        "html", "docx", "odt", "rtf", "epub", "tex", "rst", "org", "adoc", "txt", "typst", "pptx",
        "ipynb",
    ] {
        r.check(&md, to);
    }
    if let Some(docx) = r.check(&md, "docx") {
        let back = r.check(&docx, "md");
        r.expect_text(back.as_ref(), &["Başlık", "birinci madde"]);
        r.check(&docx, "pdf");
        r.check(&docx, "html");
    }
    let txt_md = r.check(&txt, "md");
    r.expect_text(txt_md.as_ref(), &["\\# not a heading", "1\\. not a list"]);

    // ---- data and spreadsheets ----------------------------------------------------------
    let json = write(
        "veri.json",
        br#"[{"ad":"elma","sayi":3},{"ad":"armut","sayi":5}]"#,
    );
    for to in ["yaml", "toml", "xml", "csv", "tsv", "xlsx"] {
        r.check(&json, to);
    }
    let csv = write("tablo.csv", "ad,sayı\nelma,3\narmut,5\n".as_bytes());
    if let Some(xlsx) = r.check(&csv, "xlsx") {
        for to in ["csv", "json", "html", "md", "tsv"] {
            r.check(&xlsx, to);
        }
        r.check(&xlsx, "pdf");
    }
    r.check(&csv, "docx");
    let xml = write("veri.xml", b"<?xml version=\"1.0\"?><kitap id=\"1\"><ad>EON &amp; Converter</ad><etiket>a</etiket><etiket>b</etiket></kitap>");
    let from_xml = r.check(&xml, "json");
    r.expect_text(from_xml.as_ref(), &["EON & Converter", "\"@id\": \"1\""]);

    // ---- images --------------------------------------------------------------------------
    let png = src.join("resim.png");
    image::RgbaImage::from_fn(320, 200, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
    })
    .save(&png)
    .unwrap();
    for to in [
        "jpg", "webp", "gif", "bmp", "tiff", "ico", "tga", "qoi", "pnm", "avif", "jxl", "jp2",
        "pdf", "psd", "pcx",
    ] {
        r.check(&png, to);
    }
    let svg = write("çizim.svg", br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle cx="50" cy="50" r="40" fill="teal"/></svg>"#);
    r.check(&svg, "png");

    // ---- video, audio, animation ------------------------------------------------------------
    let ffmpeg = r.rt.block_on(r.conv.fetch(Engine::Ffmpeg)).unwrap();
    let mp4 = src.join("klip.mp4");
    let made = std::process::Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=640x360:rate=25:duration=3",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=3",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&mp4)
        .status()
        .unwrap();
    assert!(made.success());
    let mkv = r.check(&mp4, "mkv");
    r.expect_streams(mkv.as_ref(), &["Video", "Audio"]);
    for to in [
        "webm", "mov", "avi", "wmv", "flv", "mpeg", "ts", "3gp", "ogv", "m4v",
    ] {
        let out = r.check(&mp4, to);
        r.expect_streams(out.as_ref(), &["Video", "Audio"]);
    }
    for to in [
        "mp3", "m4a", "wav", "flac", "ogg", "opus", "wma", "aiff", "ac3", "aac",
    ] {
        let out = r.check(&mp4, to);
        r.expect_streams(out.as_ref(), &["Audio"]);
    }
    let gif = r.check(&mp4, "gif");
    r.check(&mp4, "png");
    r.check(&mp4, "webp");
    if let Some(gif) = gif {
        let back = r.check(&gif, "mp4");
        r.expect_streams(back.as_ref(), &["Video"]);
        r.check(&gif, "webp");
        r.check(&gif, "png");
    }
    if let Ok(mp3) = r.convert(&mp4, "mp3") {
        for to in ["ogg", "flac", "wav", "m4a"] {
            r.check(&mp3, to);
        }
    }

    // ---- subtitles, archives -------------------------------------------------------------------
    let srt = write("altyazı.srt", "1\n00:00:01,000 --> 00:00:02,500\nMerhaba dünya\n\n2\n00:00:03,000 --> 00:00:04,000\nİkinci satır\n".as_bytes());
    let vtt = r.check(&srt, "vtt");
    r.expect_text(vtt.as_ref(), &["WEBVTT", "Merhaba dünya"]);
    r.check(&srt, "ass");
    let zip = src.join("arşiv.zip");
    {
        let mut z = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
        z.start_file("klasör/a.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        std::io::Write::write_all(&mut z, b"merhaba").unwrap();
        z.finish().unwrap();
    }
    if let Some(tgz) = r.check(&zip, "tgz") {
        r.check(&tgz, "zip");
    }
    r.check(&zip, "tar");

    println!("\n{} passed, {} failed", r.passed, r.failures.len());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(r.failures.is_empty(), "{:#?}", r.failures);
}
