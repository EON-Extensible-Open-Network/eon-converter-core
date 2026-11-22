// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! The EON Converter core (plan madde 48).
//!
//! It knows what a file is ([`format`]), what it can become and by which chain
//! of steps ([`graph`]), and how each step is done ([`engine`]): by the core
//! itself, or by an engine fetched the first time it is needed ([`tools`]). It
//! has no window: EON Converter's application and EON Download Manager both use
//! it the same way.

pub mod engine;
pub mod format;
pub mod graph;
pub(crate) mod run;
pub mod tools;

use engine::{Edge, Installed, Step};
use format::Format;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::watch;
use tools::{Engine, Tools};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Io(String),
    Network(String),
    Invalid(String),
    /// The conversion is not possible here, with the reason.
    Unsupported(String),
    /// An engine it needs is not here and cannot be fetched.
    Missing(String),
    /// An engine ran and failed; its own words.
    Engine(String),
    NoRoute {
        from: String,
        to: String,
    },
    Cancelled,
}

impl Error {
    /// A stable word for the interface to choose its message by.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io(_) => "disk",
            Error::Network(_) => "network",
            Error::Invalid(_) => "invalid",
            Error::Unsupported(_) => "unsupported",
            Error::Missing(_) => "missing",
            Error::Engine(_) => "engine",
            Error::NoRoute { .. } => "no_route",
            Error::Cancelled => "cancelled",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(m)
            | Error::Network(m)
            | Error::Invalid(m)
            | Error::Unsupported(m)
            | Error::Missing(m)
            | Error::Engine(m) => f.write_str(m),
            Error::NoRoute { from, to } => write!(f, "there is no way from {from} to {to}"),
            Error::Cancelled => f.write_str("cancelled"),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    High,
    #[default]
    Balanced,
    Small,
}

/// The few choices a conversion takes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub quality: Quality,
    /// Pictures and video no wider than this (height follows).
    pub max_width: Option<u32>,
    /// Frames per second of a GIF or animated WebP made from a video.
    pub fps: Option<u32>,
}

impl Options {
    pub fn jpeg_quality(&self) -> u8 {
        match self.quality {
            Quality::High => 95,
            Quality::Balanced => 88,
            Quality::Small => 72,
        }
    }
}

/// One step as the interface shows it.
#[derive(Debug, Clone, Serialize)]
pub struct PlanStep {
    pub from: &'static str,
    pub to: &'static str,
    pub engine: Engine,
    pub lossy: bool,
}

/// What a format can become, and how.
#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub format: &'static Format,
    pub steps: Vec<PlanStep>,
    /// Some step loses something by nature (re-encoding, layout, structure).
    pub lossy: bool,
    /// Engines that will be fetched first, with their size.
    pub fetch: Vec<tools::Status>,
    #[serde(skip)]
    route: graph::Route,
}

/// How far a conversion is.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stage", rename_all = "lowercase")]
pub enum Progress {
    /// An engine is being fetched.
    Fetching { engine: Engine },
    /// Step `step` of `steps`; `fraction` is the whole conversion's, 0 to 1.
    Converting {
        step: usize,
        steps: usize,
        engine: Engine,
        fraction: f32,
    },
}

pub struct Converter {
    tools: Tools,
    client: reqwest::Client,
    edges: Vec<Edge>,
    installed: Installed,
}

static WORK_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Converter {
    /// Engines live in `data_dir/tools`.
    pub fn new(data_dir: &Path) -> Result<Self, Error> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .user_agent(concat!("EON-Converter/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        let tools = Tools::new(data_dir);
        let installed = Installed {
            libreoffice: tools.find(Engine::LibreOffice).is_some(),
            calibre: tools.find(Engine::Calibre).is_some(),
        };
        Ok(Converter {
            edges: engine::edges(installed),
            tools,
            client,
            installed,
        })
    }

    /// LibreOffice and Calibre, which are used only if installed.
    pub fn installed(&self) -> Installed {
        self.installed
    }

    pub fn engines(&self) -> Vec<tools::Status> {
        [
            Engine::Ffmpeg,
            Engine::Pandoc,
            Engine::Typst,
            Engine::Pdfium,
            Engine::Magick,
            Engine::LibreOffice,
            Engine::Calibre,
        ]
        .into_iter()
        .map(|e| self.tools.status(e))
        .collect()
    }

    /// What a file is, from its content first and its name second.
    pub fn detect(&self, path: &Path) -> Result<&'static Format, Error> {
        format::detect_file(path)
            .map_err(|e| Error::Io(e.to_string()))?
            .ok_or_else(|| Error::Unsupported("what this file is could not be told".into()))
    }

    fn ready(&self, engine: Engine) -> bool {
        engine == Engine::Core || self.tools.find(engine).is_some()
    }

    fn target(&self, to: &'static str, route: graph::Route) -> Option<Target> {
        let format = format::by_id(to)?;
        let fetch = route
            .engines()
            .into_iter()
            .filter(|e| !self.ready(*e))
            .map(|e| self.tools.status(e))
            .collect();
        Some(Target {
            format,
            steps: route
                .steps
                .iter()
                .map(|s| PlanStep {
                    from: s.from,
                    to: s.to,
                    engine: s.engine,
                    lossy: s.lossy,
                })
                .collect(),
            lossy: route.lossy(),
            fetch,
            route,
        })
    }

    /// Every format `from` can become, in the table's order.
    pub fn targets(&self, from: &str) -> Vec<Target> {
        let routes = graph::routes(&self.edges, from, &|e| self.ready(e));
        format::FORMATS
            .iter()
            .filter_map(|f| routes.get(f.id).cloned().and_then(|r| self.target(f.id, r)))
            .collect()
    }

    /// How `from` becomes `to`.
    pub fn plan(&self, from: &str, to: &str) -> Result<Target, Error> {
        let to_static = format::by_id(to)
            .map(|f| f.id)
            .ok_or_else(|| Error::Invalid(format!("unknown format {to}")))?;
        graph::routes(&self.edges, from, &|e| self.ready(e))
            .remove(to_static)
            .and_then(|r| self.target(to_static, r))
            .ok_or_else(|| Error::NoRoute {
                from: from.into(),
                to: to.into(),
            })
    }

    /// Of the formats a source offers, the one that becomes `to` most cheaply
    /// (itself when offered), with its route. For EON Download Manager: what to
    /// download so that the conversion afterwards loses least.
    pub fn best_source(
        &self,
        offered: &[&str],
        to: &str,
    ) -> Option<(&'static str, Option<Target>)> {
        if let Some(f) = offered
            .iter()
            .find(|o| **o == to)
            .and_then(|o| format::by_id(o))
        {
            return Some((f.id, None));
        }
        offered
            .iter()
            .filter_map(|o| {
                let id = format::by_id(o)?.id;
                self.plan(id, to).ok().map(|t| (id, t))
            })
            .min_by_key(|(_, t)| (t.lossy, t.route.cost))
            .map(|(id, t)| (id, Some(t)))
    }

    /// Fetch an engine now (the application does this with a visible bar).
    pub async fn fetch(&self, engine: Engine) -> Result<PathBuf, Error> {
        self.tools.ensure(&self.client, engine).await
    }

    /// Convert `input` to `to`, into `folder` (the input's own when `None`). The
    /// result gets the input's name with the new extension, made unique. Several
    /// page pictures make a folder of that name.
    pub async fn convert(
        &self,
        input: &Path,
        to: &str,
        folder: Option<&Path>,
        options: &Options,
        progress: &(dyn Fn(Progress) + Send + Sync),
        cancel: watch::Receiver<bool>,
    ) -> Result<PathBuf, Error> {
        let from = self.detect(input)?;
        let target = self.plan(from.id, to)?;
        let folder = folder
            .map(Path::to_path_buf)
            .or_else(|| input.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        std::fs::create_dir_all(&folder).map_err(|e| Error::Io(e.to_string()))?;
        let stem = input
            .file_stem()
            .map_or_else(|| "converted".into(), |s| s.to_string_lossy().into_owned());
        let stem = stem.strip_suffix(".tar").map(str::to_owned).unwrap_or(stem);
        let ext = target.format.extensions[0];
        let dest = unique(&folder, &format!("{stem}.{ext}"));
        // Intermediate files sit beside the result, so the result moves into
        // place by renaming, never by copying a large file across drives.
        let work = folder.join(format!(
            ".eon-convert-{}-{}",
            std::process::id(),
            WORK_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&work).map_err(|e| Error::Io(e.to_string()))?;
        let result = self
            .run_steps(input, &target, &work, &dest, options, progress, cancel)
            .await;
        let _ = std::fs::remove_dir_all(&work);
        if result.is_err() {
            let _ = std::fs::remove_file(&dest);
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_steps(
        &self,
        input: &Path,
        target: &Target,
        work: &Path,
        dest: &Path,
        options: &Options,
        progress: &(dyn Fn(Progress) + Send + Sync),
        cancel: watch::Receiver<bool>,
    ) -> Result<PathBuf, Error> {
        let steps = target.route.steps.len();
        let mut current = input.to_path_buf();
        for (i, edge) in target.route.steps.iter().enumerate() {
            if *cancel.borrow() {
                return Err(Error::Cancelled);
            }
            if !self.ready(edge.engine) {
                progress(Progress::Fetching {
                    engine: edge.engine,
                });
                self.tools.ensure(&self.client, edge.engine).await?;
            }
            let from = format::by_id(edge.from).ok_or_else(|| Error::Invalid(edge.from.into()))?;
            let to = format::by_id(edge.to).ok_or_else(|| Error::Invalid(edge.to.into()))?;
            let last = i + 1 == steps;
            let out = if last {
                // Written under a working name, renamed when whole.
                work.join(dest.file_name().unwrap_or_default())
            } else {
                work.join(format!("step{i}.{}", to.extensions[0]))
            };
            // Some engines take a file by a plain name more reliably than by any
            // name a person may give: those get a copy under a plain name.
            if i == 0 && !matches!(edge.engine, Engine::Core | Engine::Ffmpeg | Engine::Pdfium) {
                let plain = work.join(format!("input.{}", from.extensions[0]));
                std::fs::copy(&current, &plain).map_err(|e| Error::Io(e.to_string()))?;
                current = plain;
            }
            let report = move |f: f32| {
                progress(Progress::Converting {
                    step: i + 1,
                    steps,
                    engine: edge.engine,
                    fraction: (i as f32 + f.clamp(0.0, 1.0)) / steps as f32,
                });
            };
            report(0.0);
            let mut step = Step {
                tools: &self.tools,
                client: &self.client,
                options,
                work,
                origin: input.parent().unwrap_or(work),
                cancel: cancel.clone(),
                report: &report,
            };
            engine::convert(&mut step, edge, from, to, &current, &out).await?;
            if out.is_file() {
                check_made(&out, to, edge.engine)?;
            }
            report(1.0);
            current = out;
        }
        place(&current, dest)
    }
}

/// An engine that cannot write a format may still exit 0: ImageMagick without an
/// encoder writes a PNG under the asked-for name. Each step's file is read back,
/// so what it holds, not what it is called, decides whether the step worked.
fn check_made(made: &Path, want: &Format, engine: Engine) -> Result<(), Error> {
    let io = |e: std::io::Error| Error::Io(e.to_string());
    if std::fs::metadata(made).map_err(io)?.len() == 0 {
        return Err(Error::Engine(format!(
            "{} wrote an empty file",
            engine.name()
        )));
    }
    match format::detect_file(made).map_err(io)? {
        Some(got) if !format::agrees(got, want) => Err(Error::Engine(format!(
            "{} wrote {} instead of {}",
            engine.name(),
            got.name,
            want.name
        ))),
        _ => Ok(()),
    }
}

/// Move a finished result (a file, a folder of page pictures, or a file with
/// its `_media` folder) from the work folder to its place.
fn place(made: &Path, dest: &Path) -> Result<PathBuf, Error> {
    let io = |e: std::io::Error| Error::Io(e.to_string());
    if made.is_file() {
        std::fs::rename(made, dest).map_err(io)?;
        let stem = |p: &Path| {
            p.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let media = made.with_file_name(format!("{}_media", stem(made)));
        if media.is_dir() {
            let _ = std::fs::rename(&media, dest.with_file_name(format!("{}_media", stem(dest))));
        }
        return Ok(dest.to_path_buf());
    }
    let pages = made.with_extension("");
    if pages.is_dir() {
        let folder = unique_dir(&dest.with_extension(""));
        std::fs::rename(&pages, &folder).map_err(io)?;
        return Ok(folder);
    }
    Err(Error::Engine("the conversion produced nothing".into()))
}

/// `name` in `folder`, or `name (1)`, `name (2)`... if it is taken.
pub fn unique(folder: &Path, name: &str) -> PathBuf {
    let first = folder.join(name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_owned(), format!(".{e}")),
        _ => (name.to_owned(), String::new()),
    };
    (1..u32::MAX)
        .map(|n| folder.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

fn unique_dir(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let folder = path.parent().unwrap_or(Path::new("."));
    if !path.exists() {
        return path.to_path_buf();
    }
    (1..u32::MAX)
        .map(|n| folder.join(format!("{name} ({n})")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_result_that_is_not_what_was_asked_is_refused() {
        let dir = std::env::temp_dir().join(format!("eon-converter-made-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        let heic = dir.join("picture.heic");
        std::fs::write(&heic, png).unwrap();
        let err = check_made(&heic, format::by_id("heic").unwrap(), Engine::Magick).unwrap_err();
        assert!(matches!(err, Error::Engine(_)), "{err:?}");

        let m4a = dir.join("song.m4a");
        std::fs::write(&m4a, b"\0\0\0\x20ftypM4A \0\0\0\0M4A mp42isom").unwrap();
        check_made(&m4a, format::by_id("m4a").unwrap(), Engine::Ffmpeg).unwrap();

        let empty = dir.join("empty.txt");
        std::fs::write(&empty, b"").unwrap();
        assert!(check_made(&empty, format::by_id("txt").unwrap(), Engine::Pandoc).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
