// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! The routing graph: formats are nodes, one engine step is an edge, and a
//! conversion is the cheapest path. Each step adds one to its cost, so a direct
//! conversion beats a detour of equal price; an engine not yet on the system
//! costs a little more, so one already here is preferred when both would do.

use crate::engine::Edge;
use crate::tools::Engine;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Each step's cost on top of its edge's own.
const PER_STEP: u32 = 1;
/// The price of an engine that would have to be fetched first.
const TO_FETCH: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub steps: Vec<Edge>,
    pub cost: u32,
}

impl Route {
    /// Some step loses something.
    pub fn lossy(&self) -> bool {
        self.steps.iter().any(|s| s.lossy)
    }

    /// The engines it needs, in order, each once.
    pub fn engines(&self) -> Vec<Engine> {
        let mut out = Vec::new();
        for s in &self.steps {
            if !out.contains(&s.engine) {
                out.push(s.engine);
            }
        }
        out
    }
}

/// A step after which nothing else can run (its result may be a folder).
fn terminal(edge: &Edge) -> bool {
    edge.engine == Engine::Pdfium && matches!(edge.to, "png" | "jpg")
}

/// The cheapest route from `from` to every format it can reach.
pub fn routes(
    edges: &[Edge],
    from: &str,
    ready: &dyn Fn(Engine) -> bool,
) -> HashMap<&'static str, Route> {
    let mut best: HashMap<&'static str, (u32, Vec<Edge>, bool)> = HashMap::new();
    let mut queue: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
    // Paths are kept by index so the heap holds only numbers.
    let mut paths: Vec<(&'static str, Vec<Edge>, bool)> = Vec::new();
    paths.push(("", Vec::new(), false));
    queue.push(Reverse((0, 0)));
    while let Some(Reverse((cost, index))) = queue.pop() {
        let (node, path, ended) = paths[index].clone();
        let here = if index == 0 { from } else { node };
        if index != 0 {
            if best.get(node).is_some_and(|(c, _, _)| *c <= cost) {
                continue;
            }
            best.insert(node, (cost, path.clone(), ended));
        }
        if ended {
            continue;
        }
        for edge in edges.iter().filter(|e| e.from == here) {
            if edge.to == from || path.iter().any(|s| s.from == edge.to) {
                continue;
            }
            let price = cost + edge.cost + PER_STEP + if ready(edge.engine) { 0 } else { TO_FETCH };
            if best.get(edge.to).is_some_and(|(c, _, _)| *c <= price) {
                continue;
            }
            let mut next = path.clone();
            next.push(*edge);
            paths.push((edge.to, next, terminal(edge)));
            queue.push(Reverse((price, paths.len() - 1)));
        }
    }
    best.into_iter()
        .map(|(to, (cost, steps, _))| (to, Route { steps, cost }))
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::engine::{edges, Installed};

    fn all_ready(_: Engine) -> bool {
        true
    }

    #[test]
    fn the_owners_examples_have_routes() {
        let e = edges(Installed::default());
        let txt = routes(&e, "txt", &all_ready);
        let to_pdf: Vec<_> = txt["pdf"].steps.iter().map(|s| (s.from, s.to)).collect();
        assert_eq!(
            to_pdf,
            vec![("txt", "md"), ("md", "typst"), ("typst", "pdf")]
        );
        let pdf = routes(&e, "pdf", &all_ready);
        assert_eq!(pdf["md"].steps.len(), 1, "PDF to Markdown is one step");
        assert!(pdf["md"].lossy(), "and says it loses something");
        assert_eq!(
            pdf["docx"].steps.last().unwrap().from,
            "md",
            "PDF to Word goes through Markdown"
        );
        let mp4 = routes(&e, "mp4", &all_ready);
        assert_eq!(mp4["gif"].steps.len(), 1);
        assert_eq!(routes(&e, "gif", &all_ready)["mp4"].steps.len(), 1);
        assert!(!mp4["mkv"].lossy(), "a container change keeps everything");
    }

    #[test]
    fn a_page_picture_is_a_last_step() {
        let e = edges(Installed::default());
        let pdf = routes(&e, "pdf", &all_ready);
        assert!(pdf.contains_key("png"));
        assert!(!pdf.contains_key("ico"), "no step after a page picture");
    }

    #[test]
    fn an_engine_already_here_is_preferred() {
        let e = edges(Installed::default());
        // PNG to AVIF: FFmpeg and ImageMagick both can; only FFmpeg is here.
        let only_ffmpeg = |x: Engine| x == Engine::Ffmpeg || x == Engine::Core;
        assert_eq!(
            routes(&e, "png", &only_ffmpeg)["avif"].engines(),
            vec![Engine::Ffmpeg]
        );
        let only_magick = |x: Engine| x == Engine::Magick || x == Engine::Core;
        assert_eq!(
            routes(&e, "png", &only_magick)["avif"].engines(),
            vec![Engine::Magick]
        );
    }

    #[test]
    fn heic_is_read_never_written() {
        // The pinned ImageMagick reads HEIC but has no encoder for it, and on a
        // missing encoder it writes a PNG under the .heic name and exits 0.
        let e = edges(Installed::default());
        assert!(routes(&e, "heic", &all_ready).contains_key("png"));
        assert!(!routes(&e, "png", &all_ready).contains_key("heic"));
    }
}
