<!--
SPDX-License-Identifier: GPL-3.0-or-later
SPDX-FileCopyrightText: 2026 EON contributors
-->

# Security

Report privately through GitHub's **Security → Report a vulnerability** on this
repository; the organisation-wide policy is
[`.github/SECURITY.md`](https://github.com/EON-Extensible-Open-Network/.github/blob/main/SECURITY.md).

## What a converter has to get right

- **The file decides what it is, not its name.** Detection reads the first bytes before
  trusting the extension (`src/format.rs`).
- **An archive cannot write outside its folder.** ZIP entries go through
  `enclosed_name`; TAR entries with a `..` component or an absolute path are refused
  (`src/engine/core.rs`).
- **A result never replaces a file.** It gets the input's name with the new extension,
  made unique, and is written under a working name in a folder beside it, renamed only
  when whole. A failed or cancelled conversion leaves nothing behind.
- **Engines are fetched, never bundled, and checked.** FFmpeg, Pandoc, Typst, PDFium and
  ImageMagick are downloaded the first time they are needed from their own projects'
  release pages, streamed to disk while their SHA-256 is computed, and kept only if it
  matches the pin in `src/tools.rs`. A file that does not match is deleted and never run.
- **Engines run without a console window, without standard input, and end with their
  owner** (`src/run.rs`). Input paths follow the flag that takes them, or are copied under
  a plain name first, so a file name is never read as an option.
- **LibreOffice runs with a profile of its own** in the work folder, so a copy the person
  has open is not disturbed and its settings are not read.
- **No DRM is decrypted.** No engine is ever given a key, so a protected file fails
  instead of converting.
- **No telemetry** — CI checks for it.

## Known

- Pandoc follows references inside the document it reads: an HTML or Markdown file
  converted to a self-contained format may make Pandoc fetch the pictures it links to.
  Convert a document you did not write as you would open it.
- The engines are large C and C++ programs reading untrusted files. They run with the
  person's rights; there is no sandbox yet (plan madde 4 for the shape one would take).
- Builds are not signed (no keys yet; plan madde 30, 34).
