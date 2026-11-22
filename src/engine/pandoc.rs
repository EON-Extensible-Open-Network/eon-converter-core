// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Documents: Pandoc between every markup and office format it knows, and
//! Typst for PDF (Pandoc writes Typst, Typst writes the PDF: no LaTeX install).

use super::{pairs, Edge, Step};
use crate::format::Format;
use crate::tools::Engine;
use crate::{run, Error};
use std::path::Path;

const READS: &[&str] = &[
    "md",
    "gfm",
    "html",
    "docx",
    "odt",
    "rtf",
    "epub",
    "tex",
    "rst",
    "org",
    "textile",
    "mediawiki",
    "dokuwiki",
    "jira",
    "fb2",
    "docbook",
    "ipynb",
    "typst",
    "csv",
    "tsv",
];
const WRITES: &[&str] = &[
    "md",
    "gfm",
    "html",
    "docx",
    "odt",
    "rtf",
    "epub",
    "tex",
    "rst",
    "org",
    "adoc",
    "textile",
    "mediawiki",
    "dokuwiki",
    "jira",
    "fb2",
    "docbook",
    "ipynb",
    "typst",
    "man",
    "pptx",
    "txt",
];
/// Formats with layout and styling that plain markup does not carry.
const RICH: &[&str] = &[
    "docx", "odt", "rtf", "html", "epub", "fb2", "docbook", "tex", "pptx",
];

pub fn edges(all: &mut Vec<Edge>) {
    pairs(all, Engine::Pandoc, READS, WRITES, 3, |f, t| {
        t == "txt" || (RICH.contains(&f) && !RICH.contains(&t))
    });
    all.push(Edge {
        from: "typst",
        to: "pdf",
        engine: Engine::Typst,
        cost: 2,
        lossy: false,
    });
}

fn reader(id: &str) -> &str {
    match id {
        "md" => "markdown",
        "tex" => "latex",
        other => other,
    }
}

fn writer(id: &str) -> &str {
    match id {
        "md" => "markdown",
        "html" => "html5",
        "epub" => "epub3",
        "tex" => "latex",
        "adoc" => "asciidoc",
        "docbook" => "docbook5",
        "txt" => "plain",
        other => other,
    }
}

pub async fn convert(
    step: &mut Step<'_>,
    engine: Engine,
    from: &Format,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    if engine == Engine::Typst {
        let typst = step.engine(Engine::Typst).await?;
        // The root covers the folder the Typst file and its pictures are in.
        let root = input.parent().unwrap_or(step.work);
        let mut cmd = run::command(&typst);
        cmd.arg("compile")
            .arg("--root")
            .arg(root)
            .arg(input)
            .arg(output);
        return run::run(cmd, &mut step.cancel, &|_| {}).await;
    }
    let pandoc = step.engine(Engine::Pandoc).await?;
    let mut cmd = run::command(&pandoc);
    cmd.args([
        "-f",
        reader(from.id),
        "-t",
        writer(to.id),
        "--wrap=preserve",
    ]);
    if !matches!(to.id, "docx" | "odt" | "epub" | "pptx") {
        cmd.arg("--standalone");
    }
    cmd.arg(format!("--resource-path={}", step.origin.display()));
    match to.id {
        // One file the person can move: pictures inside it.
        "html" => {
            cmd.arg("--embed-resources");
        }
        // On the way to a PDF: every picture, linked or contained, copied beside
        // the Typst file, where Typst's root can reach it.
        "typst" => {
            let media = output.parent().unwrap_or(step.work).join("media");
            cmd.arg(format!("--extract-media={}", media.display()));
        }
        // Text formats keep pictures in a folder beside the file.
        "docx" | "odt" | "epub" | "pptx" | "rtf" | "txt" => {}
        _ if RICH.contains(&from.id) => {
            let base = output
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let media = output.with_file_name(format!("{base}_media"));
            cmd.arg(format!("--extract-media={}", media.display()));
        }
        _ => {}
    }
    cmd.arg("-o").arg(output).arg(input);
    (step.report)(0.0);
    run::run(cmd, &mut step.cancel, &|_| {}).await
}
