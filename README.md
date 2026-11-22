<!--
SPDX-License-Identifier: GPL-3.0-or-later
SPDX-FileCopyrightText: 2026 EON contributors
-->

# eon-converter-core

The core of **EON Converter** (plan madde 48): it tells what a file is, finds every
format it can become, and converts it through the cheapest chain of steps. It is a
library with no window. EON Converter's application uses it, and EON Download Manager
will too, to offer every format a download can become.

```rust
let core = Converter::new(data_dir)?;
let pdf  = core.detect(Path::new("notes.txt"))?;     // by content, then by name
let all  = core.targets(pdf.id);                      // every reachable format, with its route
let out  = core.convert(input, "pdf", None, &Options::default(), &|p| {}, cancel).await?;
let (src, _) = core.best_source(&["webm", "mp4"], "mov").unwrap(); // "mp4": a remux, no re-encode
```

## How it converts

No single program converts everything. The core is a router over proven engines. Each
step is an edge in a graph, and a conversion is the cheapest path:

| Engine | What it does | Fetched |
|---|---|---|
| The core itself (Rust) | JSON, YAML, TOML, XML, CSV, TSV; XLSX/XLS/XLSB/ODS read and XLSX written; text to Markdown; PNG, JPEG, GIF, BMP, ICO, TIFF, WebP, TGA, QOI, PNM; ZIP, TAR, TAR.GZ | never: built in |
| FFmpeg 8.1 | video, audio, subtitles, GIF and animated WebP from video and back, AVIF, JPEG XL | first use, ~190 MB |
| Pandoc 3.14 | Markdown, HTML, DOCX, ODT, RTF, EPUB, LaTeX, reST, Org, AsciiDoc and more | first use, 44 MB |
| Typst 0.15.1 | PDF from any document Pandoc reads (no LaTeX needed) | first use, 22 MB |
| PDFium | PDF to text, to Markdown, to page pictures | first use, 4 MB |
| ImageMagick 7.1.2 | PSD, SVG, JPEG 2000, PCX, XCF, and HEIC (read only: no pinned engine writes it); animated GIF ↔ WebP; pictures to PDF | first use, 13 MB |
| LibreOffice | office files with their layout kept (DOC, DOCX, XLS, PPTX... ↔ PDF) | only if installed |
| Calibre | MOBI, AZW3 and other ebooks | only if installed |

Every fetched engine is pinned by version and SHA-256 in `src/tools.rs` and comes from its
own project's release page. A file that does not match its pin is deleted and never run.

## What it does not promise

Some conversions lose things by nature, and each step that does is marked `lossy` before
anything runs. Each step's file is read back afterwards: an engine that exits cleanly but
wrote something other than what was asked fails the conversion instead of passing it on. PDF to Markdown rebuilds headings, paragraphs and lists from the page;
tables and columns come back only as text in reading order. A scanned PDF has no text
without OCR, which is not built in yet. A GIF has 256 colours. No DRM is decrypted: a
protected file fails instead of converting.

## Checks

```
cargo test                                          # detection, routing, the core's own conversions
cargo test --test matrix -- --ignored --nocapture   # real files of every family through the real engines
```

The matrix fetches the engines into `target/converter-test-tools` the first time (about
280 MB). It runs in CI on Windows.

Licence: `GPL-3.0-or-later` with the EON Module ABI Exception (`LICENSE-EXCEPTION.md`).
The engines keep their own licences; they are separate programs, or a library loaded at
run time (PDFium), never linked into this crate.
