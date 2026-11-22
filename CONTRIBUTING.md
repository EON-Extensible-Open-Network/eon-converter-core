<!--
SPDX-License-Identifier: GPL-3.0-or-later
SPDX-FileCopyrightText: 2026 EON contributors
-->

# Contributing to EON Converter core

The organisation-wide guide,
[`.github/CONTRIBUTING.md`](https://github.com/EON-Extensible-Open-Network/.github/blob/main/CONTRIBUTING.md),
applies: DCO sign-off (`git commit -s`), Conventional Commits (`feat(engine): ...`,
`fix(graph): ...`, `feat(format): ...`). By contributing you license your work under
`GPL-3.0-or-later` with the EON Module ABI Exception.

- **A library, never a window.** No GUI dependency and no listening socket; CI fails
  on either. EON Converter's application and EON Download Manager link this crate.
- **A new format is a row in `src/format.rs`** with its content signature where it has
  one; detection by content comes before the name.
- **A new conversion is an edge in `src/engine/mod.rs`** with an honest cost and an
  honest `lossy` flag, and a case in `tests/matrix.rs` that runs it on a real file.
- **A new engine is pinned** by version and SHA-256 in `src/tools.rs`, fetched from its
  own project's release, and never bundled. Its licence goes in `NOTICE`.
- **An input never becomes an option.** Pass paths after the flag that takes them, or
  copy the file under a plain name, as `run_steps` does.
- **No telemetry** (madde 36). CI checks for it.
