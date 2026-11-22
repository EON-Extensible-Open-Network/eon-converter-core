// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! What each engine can do in one step (the graph's edges) and how it does it.
//!
//! An edge's cost is how much the step is to be avoided: a container change
//! costs little, a re-encode more, a step that drops structure (PDF to text)
//! most. `lossy` marks a step that loses something by nature; the application
//! shows it before converting.

pub mod core;
pub mod ffmpeg;
pub mod magick;
pub mod office;
pub mod pandoc;
pub mod pdf;

use crate::format::Format;
use crate::tools::{Engine, Tools};
use crate::{Error, Options};
use serde::Serialize;
use std::path::Path;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Edge {
    pub from: &'static str,
    pub to: &'static str,
    pub engine: Engine,
    pub cost: u32,
    pub lossy: bool,
}

/// Engines that cannot be fetched and are present on this system.
#[derive(Debug, Clone, Copy, Default)]
pub struct Installed {
    pub libreoffice: bool,
    pub calibre: bool,
}

/// Every edge, given which installed-only engines are present.
pub fn edges(installed: Installed) -> Vec<Edge> {
    let mut all = Vec::new();
    core::edges(&mut all);
    ffmpeg::edges(&mut all);
    pandoc::edges(&mut all);
    pdf::edges(&mut all);
    magick::edges(&mut all);
    if installed.libreoffice {
        office::libreoffice_edges(&mut all);
    }
    if installed.calibre {
        office::calibre_edges(&mut all);
    }
    all
}

/// Add an edge for every pair of `from` × `to` (a format never to itself).
pub(crate) fn pairs(
    all: &mut Vec<Edge>,
    engine: Engine,
    from: &[&'static str],
    to: &[&'static str],
    cost: u32,
    lossy: impl Fn(&str, &str) -> bool,
) {
    for f in from {
        for t in to {
            if f != t {
                all.push(Edge {
                    from: f,
                    to: t,
                    engine,
                    cost,
                    lossy: lossy(f, t),
                });
            }
        }
    }
}

/// What a step has to work with.
pub struct Step<'a> {
    pub tools: &'a Tools,
    pub client: &'a reqwest::Client,
    pub options: &'a Options,
    /// A folder of its own for intermediate files, deleted after the conversion.
    pub work: &'a Path,
    /// The folder the original input is in, where its linked files are.
    pub origin: &'a Path,
    pub cancel: watch::Receiver<bool>,
    /// The step's own progress, 0.0 to 1.0, when the engine tells.
    pub report: &'a (dyn Fn(f32) + Send + Sync),
}

impl Step<'_> {
    pub async fn engine(&self, engine: Engine) -> Result<std::path::PathBuf, Error> {
        self.tools.ensure(self.client, engine).await
    }
}

/// Run one edge.
pub async fn convert(
    step: &mut Step<'_>,
    edge: &Edge,
    from: &Format,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    match edge.engine {
        Engine::Core => core::convert(step, from, to, input, output).await,
        Engine::Ffmpeg => ffmpeg::convert(step, from, to, input, output).await,
        Engine::Pandoc | Engine::Typst => {
            pandoc::convert(step, edge.engine, from, to, input, output).await
        }
        Engine::Pdfium => pdf::convert(step, to, input, output).await,
        Engine::Magick => magick::convert(step, from, to, input, output).await,
        Engine::LibreOffice => office::libreoffice(step, to, input, output).await,
        Engine::Calibre => office::calibre(step, input, output).await,
    }
}
