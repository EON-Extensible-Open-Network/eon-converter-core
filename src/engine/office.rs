// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! LibreOffice and Calibre, when the person has them installed: office files
//! with their layout kept, and ebook formats. Neither is fetched (each needs an
//! administrator install), so their edges exist only when they are found.

use super::{pairs, Edge, Step};
use crate::format::Format;
use crate::tools::Engine;
use crate::{run, Error};
use std::path::Path;

const WRITER_IN: &[&str] = &["doc", "docx", "odt", "rtf", "html"];
const WRITER_OUT: &[&str] = &["pdf", "docx", "doc", "odt", "rtf"];
const CALC_IN: &[&str] = &["xls", "xlsx", "xlsb", "ods", "csv"];
const CALC_OUT: &[&str] = &["pdf", "xlsx", "xls", "ods", "csv"];
const IMPRESS_IN: &[&str] = &["ppt", "pptx", "odp"];
const IMPRESS_OUT: &[&str] = &["pdf", "pptx", "ppt", "odp"];
const EBOOK_IN: &[&str] = &["epub", "mobi", "azw3", "fb2", "docx", "html", "pdf"];
const EBOOK_OUT: &[&str] = &["epub", "mobi", "azw3", "fb2", "pdf", "docx", "txt"];

pub fn libreoffice_edges(all: &mut Vec<Edge>) {
    let e = Engine::LibreOffice;
    pairs(all, e, WRITER_IN, WRITER_OUT, 2, |_, t| {
        t == "doc" || t == "rtf"
    });
    pairs(all, e, CALC_IN, CALC_OUT, 2, |_, t| {
        t == "csv" || t == "xls"
    });
    pairs(all, e, IMPRESS_IN, IMPRESS_OUT, 2, |_, t| t == "ppt");
}

pub fn calibre_edges(all: &mut Vec<Edge>) {
    pairs(all, Engine::Calibre, EBOOK_IN, EBOOK_OUT, 5, |f, t| {
        f == "pdf" || t == "txt"
    });
}

/// LibreOffice's export filter where the extension alone is not enough.
fn filter(to: &str) -> String {
    match to {
        // UTF-8, comma-separated, quoted text.
        "csv" => "csv:Text - txt - csv (StarCalc):44,34,76".into(),
        "doc" => "doc:MS Word 97".into(),
        "xls" => "xls:MS Excel 97".into(),
        "ppt" => "ppt:MS PowerPoint 97".into(),
        other => other.into(),
    }
}

pub async fn libreoffice(
    step: &mut Step<'_>,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    let soffice = step.engine(Engine::LibreOffice).await?;
    let out_dir = step.work.join("libreoffice-out");
    std::fs::create_dir_all(&out_dir).map_err(|e| Error::Io(e.to_string()))?;
    // A profile of its own, so a LibreOffice the person has open is not disturbed.
    let profile = step.work.join("libreoffice-profile");
    let profile_url = format!(
        "file:///{}",
        profile
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    );
    let mut cmd = run::command(&soffice);
    cmd.arg(format!("-env:UserInstallation={profile_url}"))
        .args(["--headless", "--norestore", "--nologo", "--convert-to"])
        .arg(filter(to.id))
        .arg("--outdir")
        .arg(&out_dir)
        .arg(input);
    (step.report)(0.0);
    run::run(cmd, &mut step.cancel, &|_| {}).await?;
    let base = input
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let made = std::fs::read_dir(&out_dir)
        .map_err(|e| Error::Io(e.to_string()))?
        .flatten()
        .map(|e| e.path())
        .find(|p| p.file_stem().is_some_and(|s| s.to_string_lossy() == base))
        .ok_or_else(|| Error::Engine("LibreOffice wrote nothing".into()))?;
    std::fs::rename(&made, output)
        .or_else(|_| std::fs::copy(&made, output).map(|_| ()))
        .map_err(|e| Error::Io(e.to_string()))
}

pub async fn calibre(step: &mut Step<'_>, input: &Path, output: &Path) -> Result<(), Error> {
    let convert = step.engine(Engine::Calibre).await?;
    let mut cmd = run::command(&convert);
    cmd.arg(input).arg(output);
    let report = step.report;
    run::run(cmd, &mut step.cancel, &|line: &str| {
        // "34% Converting input to HTML..."
        if let Some(p) = line
            .split('%')
            .next()
            .and_then(|n| n.trim().parse::<f32>().ok())
        {
            report((p / 100.0).clamp(0.0, 1.0));
        }
    })
    .await
}
