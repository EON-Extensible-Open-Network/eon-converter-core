// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Running an engine: no console window, cancellable, its last error line kept
//! as the reason when it fails.

use crate::Error;
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;

/// A command for an engine: no console window on Windows, ended with its owner.
pub fn command(program: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdin(Stdio::null());
    cmd.kill_on_drop(true);
    cmd
}

/// Run to the end, or until `cancel` turns true. `lines` sees every line the
/// program writes (both streams), for progress.
pub async fn run(
    mut cmd: tokio::process::Command,
    cancel: &mut watch::Receiver<bool>,
    lines: &(dyn Fn(&str) + Send + Sync),
) -> Result<(), Error> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| Error::Io(e.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::Io("no output".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::Io("no errors".into()))?;
    let mut out = BufReader::new(stdout).split(b'\n');
    let mut err = BufReader::new(stderr).split(b'\n');
    let mut last_error = String::new();
    let (mut out_open, mut err_open) = (true, true);
    while out_open || err_open {
        tokio::select! {
            line = out.next_segment(), if out_open => match line {
                Ok(Some(l)) => lines(String::from_utf8_lossy(&l).trim_end()),
                _ => out_open = false,
            },
            line = err.next_segment(), if err_open => match line {
                Ok(Some(l)) => {
                    let text = String::from_utf8_lossy(&l).trim_end().to_owned();
                    lines(&text);
                    if !text.trim().is_empty() {
                        last_error = text;
                    }
                }
                _ => err_open = false,
            },
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    let _ = child.kill().await;
                    return Err(Error::Cancelled);
                }
            }
        }
    }
    let status = child.wait().await.map_err(|e| Error::Io(e.to_string()))?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::Engine(if last_error.is_empty() {
            format!("the engine stopped ({status})")
        } else {
            last_error
        }))
    }
}
