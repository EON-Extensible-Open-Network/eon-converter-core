// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! ImageMagick: the image formats the core and FFmpeg do not cover (PSD, HEIC,
//! JPEG 2000, SVG, XCF, PCX...), animated GIF and WebP into each other with
//! their animation, and pictures into a PDF.

use super::{pairs, Edge, Step};
use crate::format::Format;
use crate::tools::Engine;
use crate::{run, Error};
use std::path::Path;

const READS: &[&str] = &[
    "png", "jpg", "webp", "gif", "bmp", "tiff", "ico", "avif", "heic", "jxl", "jp2", "tga", "pnm",
    "dds", "psd", "svg", "pcx", "xcf", "qoi",
];
// No HEIC: the pinned build reads it but has no encoder, and on a missing
// encoder ImageMagick writes a PNG under the asked-for name and exits 0.
const WRITES: &[&str] = &[
    "png", "jpg", "webp", "gif", "bmp", "tiff", "ico", "avif", "jxl", "jp2", "tga", "pnm", "pcx",
    "psd", "pdf",
];
/// Targets that hold several frames; others take the first.
const MULTI: &[&str] = &["gif", "webp", "tiff", "pdf"];

pub fn edges(all: &mut Vec<Edge>) {
    pairs(all, Engine::Magick, READS, WRITES, 4, |f, t| {
        ["jpg", "gif", "ico", "avif"].contains(&t) || ["psd", "xcf", "svg"].contains(&f)
    });
}

pub async fn convert(
    step: &mut Step<'_>,
    from: &Format,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    let magick = step.engine(Engine::Magick).await?;
    let mut cmd = run::command(&magick);
    if from.id == "svg" {
        cmd.args(["-background", "none", "-density", "192"]);
    }
    // A layered or animated source into a single-picture format: its first frame
    // (for PSD and XCF, the flattened picture).
    let mut source = input.as_os_str().to_owned();
    if !MULTI.contains(&to.id)
        && ["gif", "webp", "tiff", "psd", "xcf", "ico", "heic"].contains(&from.id)
    {
        source.push("[0]");
    }
    cmd.arg(source);
    match to.id {
        "jpg" => {
            cmd.args([
                "-quality",
                &step.options.jpeg_quality().to_string(),
                "-background",
                "white",
                "-flatten",
            ]);
        }
        "avif" => {
            cmd.args(["-quality", "80"]);
        }
        "ico" => {
            cmd.args([
                "-resize",
                "256x256>",
                "-define",
                "icon:auto-resize=256,128,64,48,32,16",
            ]);
        }
        "webp" if from.id == "gif" => {
            cmd.args(["-coalesce", "-quality", "90"]);
        }
        "gif" if from.id == "webp" => {
            cmd.args(["-coalesce", "-layers", "Optimize"]);
        }
        _ => {}
    }
    cmd.arg(output);
    (step.report)(0.0);
    run::run(cmd, &mut step.cancel, &|_| {}).await
}
