<!--
SPDX-License-Identifier: GPL-3.0-or-later
SPDX-FileCopyrightText: 2026 EON contributors
-->

# Changelog

## v0.0.0 — 22 November 2026

The first version (plan madde 48).

- **97 formats in ten families:** video, audio, image, document, spreadsheet,
  presentation, ebook, data, subtitle and archive. Each format is detected by its
  content first and its name second.
- **A routing graph** finds the cheapest chain of steps to every reachable format. It
  prefers fewer steps and engines already on the system, and marks every step that loses
  something.
- **Conversions the core does itself:** data formats, spreadsheets, text to Markdown,
  common images and archives.
- **Engines fetched on first use, pinned by SHA-256:** FFmpeg, Pandoc, Typst, PDFium and
  ImageMagick. LibreOffice and Calibre are used when installed.
- **PDF to Markdown** rebuilds headings from type size, lists from their markers and
  paragraphs from line spacing. It joins words split at a line end and leaves out
  running headers and page numbers.
- **`best_source`** answers, for EON Download Manager, which of the formats a download
  offers converts to the wanted one with the least loss.
- **Every step is read back.** A file whose content is not the format asked for, or an
  empty one, fails the conversion, even when the engine said it succeeded.
