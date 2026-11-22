// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! FFmpeg: video, audio, subtitles and animated images. A container change is
//! tried as a copy first (no quality lost, seconds instead of minutes); only if
//! that fails is anything re-encoded.

use super::{pairs, Edge, Step};
use crate::format::Format;
use crate::tools::Engine;
use crate::{run, Error, Quality};
use std::path::Path;

pub const VIDEO: &[&str] = &[
    "mp4", "mkv", "webm", "mov", "avi", "wmv", "flv", "mpeg", "ts", "m4v", "3gp", "ogv",
];
pub const AUDIO: &[&str] = &[
    "mp3", "aac", "m4a", "wav", "flac", "ogg", "opus", "wma", "aiff", "ac3", "alac", "amr", "mka",
];
const ANIMATED: &[&str] = &["gif", "apng", "webp"];
const STILL: &[&str] = &["png", "jpg", "webp", "bmp", "tiff", "avif", "jxl"];
const SUBTITLES: &[&str] = &["srt", "vtt", "ass"];
/// Containers that usually take each other's streams as they are.
const REMUX: &[&str] = &["mp4", "mkv", "mov", "m4v", "ts"];
const LOSSLESS_AUDIO: &[&str] = &["wav", "flac", "aiff", "alac"];

pub fn edges(all: &mut Vec<Edge>) {
    let e = Engine::Ffmpeg;
    pairs(all, e, VIDEO, VIDEO, 4, |f, t| {
        !(REMUX.contains(&f) && REMUX.contains(&t))
    });
    pairs(all, e, VIDEO, AUDIO, 3, |_, t| !LOSSLESS_AUDIO.contains(&t));
    pairs(all, e, AUDIO, AUDIO, 3, |_, t| !LOSSLESS_AUDIO.contains(&t));
    pairs(all, e, VIDEO, ANIMATED, 6, |_, _| true);
    pairs(all, e, ANIMATED, VIDEO, 5, |_, _| true);
    // One picture from a video: the most representative frame.
    pairs(all, e, VIDEO, &["png", "jpg"], 8, |_, _| true);
    // Still images FFmpeg encodes that the core does not (AVIF, JPEG XL).
    pairs(all, e, STILL, &["avif", "jxl"], 4, |_, _| true);
    pairs(
        all,
        e,
        &["avif", "jxl"],
        &["png", "jpg", "webp", "bmp", "tiff"],
        3,
        |_, t| t == "jpg",
    );
    pairs(all, e, SUBTITLES, SUBTITLES, 2, |f, _| f == "ass");
}

/// The arguments for a re-encode into `to`.
#[allow(clippy::too_many_lines)] // one table: a target, its encoder
fn encode(to: &str, quality: Quality) -> Vec<String> {
    let crf = match quality {
        Quality::High => "18",
        Quality::Balanced => "23",
        Quality::Small => "28",
    };
    let abr = match quality {
        Quality::High => "256k",
        Quality::Balanced => "192k",
        Quality::Small => "128k",
    };
    let even = "scale=trunc(iw/2)*2:trunc(ih/2)*2".to_owned();
    let v = |args: &[&str]| args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    match to {
        "mp4" | "mov" | "m4v" | "mkv" | "ts" | "flv" => {
            let mut a = v(&[
                "-c:v", "libx264", "-preset", "medium", "-crf", crf, "-pix_fmt", "yuv420p", "-vf",
            ]);
            a.push(even);
            a.extend(v(&["-c:a", "aac", "-b:a", abr]));
            if matches!(to, "mp4" | "mov" | "m4v") {
                a.extend(v(&["-movflags", "+faststart"]));
            }
            a
        }
        "webm" => v(&[
            "-c:v",
            "libvpx-vp9",
            "-crf",
            if crf == "18" {
                "24"
            } else if crf == "23" {
                "32"
            } else {
                "40"
            },
            "-b:v",
            "0",
            "-row-mt",
            "1",
            "-deadline",
            "good",
            "-cpu-used",
            "4",
            "-c:a",
            "libopus",
            "-b:a",
            "128k",
        ]),
        "avi" => v(&[
            "-c:v",
            "mpeg4",
            "-q:v",
            "3",
            "-c:a",
            "libmp3lame",
            "-q:a",
            "2",
        ]),
        "wmv" => v(&["-c:v", "wmv2", "-b:v", "4M", "-c:a", "wmav2", "-b:a", abr]),
        "mpeg" => v(&[
            "-c:v",
            "mpeg2video",
            "-q:v",
            "3",
            "-c:a",
            "mp2",
            "-b:a",
            abr,
        ]),
        "3gp" => {
            let mut a = v(&[
                "-c:v",
                "libx264",
                "-profile:v",
                "baseline",
                "-crf",
                crf,
                "-pix_fmt",
                "yuv420p",
                "-vf",
            ]);
            a.push(even);
            a.extend(v(&["-c:a", "aac", "-b:a", "96k", "-ac", "1"]));
            a
        }
        "ogv" => v(&[
            "-c:v",
            "libtheora",
            "-q:v",
            "7",
            "-c:a",
            "libvorbis",
            "-q:a",
            "5",
        ]),
        "mp3" => v(&[
            "-vn",
            "-c:a",
            "libmp3lame",
            "-q:a",
            if quality == Quality::Small { "5" } else { "2" },
        ]),
        "aac" | "m4a" => v(&["-vn", "-c:a", "aac", "-b:a", abr]),
        "wav" => v(&["-vn", "-c:a", "pcm_s16le"]),
        "flac" | "mka" => v(&["-vn", "-c:a", "flac"]),
        "ogg" => v(&["-vn", "-c:a", "libvorbis", "-q:a", "5"]),
        "opus" => v(&["-vn", "-c:a", "libopus", "-b:a", "128k"]),
        "wma" => v(&["-vn", "-c:a", "wmav2", "-b:a", abr]),
        "aiff" => v(&["-vn", "-c:a", "pcm_s16be"]),
        "ac3" => v(&["-vn", "-c:a", "ac3", "-b:a", "384k"]),
        "alac" => v(&["-vn", "-c:a", "alac", "-f", "ipod"]),
        "amr" => v(&[
            "-vn",
            "-ar",
            "8000",
            "-ac",
            "1",
            "-c:a",
            "libopencore_amrnb",
            "-b:a",
            "12.2k",
        ]),
        _ => Vec::new(),
    }
}

/// The muxer when the extension alone does not name it.
fn muxer(to: &str) -> Option<&'static str> {
    Some(match to {
        "alac" | "m4a" => "ipod",
        "aac" => "adts",
        "mka" => "matroska",
        "ts" => "mpegts",
        "mpeg" => "mpeg",
        "3gp" => "3gp",
        "apng" => "apng",
        "ass" => "ass",
        _ => return None,
    })
}

#[allow(clippy::too_many_lines)] // the kinds of FFmpeg conversion, one branch each
pub async fn convert(
    step: &mut Step<'_>,
    from: &Format,
    to: &Format,
    input: &Path,
    output: &Path,
) -> Result<(), Error> {
    let ffmpeg = step.engine(Engine::Ffmpeg).await?;
    let (f, t) = (from.id, to.id);
    let quality = step.options.quality;
    let width = step.options.max_width;
    let mut tries: Vec<Vec<String>> = Vec::new();
    let v = |args: &[&str]| {
        args.iter()
            .map(|s| (*s).to_owned())
            .collect::<Vec<String>>()
    };

    if VIDEO.contains(&t) && VIDEO.contains(&f) {
        if REMUX.contains(&f) && REMUX.contains(&t) && width.is_none() {
            let mut copy = v(&["-map", "0:v?", "-map", "0:a?", "-c", "copy"]);
            if matches!(t, "mp4" | "mov" | "m4v") {
                copy.extend(v(&["-movflags", "+faststart"]));
            }
            tries.push(copy);
        }
        let mut enc = v(&["-map", "0:v:0?", "-map", "0:a?"]);
        enc.extend(scaled(encode(t, quality), width));
        tries.push(enc);
    } else if AUDIO.contains(&t) {
        // The sound as it is, when the target container takes it.
        if matches!(
            (f, t),
            ("m4a" | "aac" | "mp4" | "mov" | "m4v", "m4a" | "aac")
                | ("mp3", "mp3")
                | ("ogg" | "webm" | "mkv", "mka")
        ) {
            tries.push(v(&["-vn", "-c:a", "copy"]));
        }
        tries.push(encode(t, quality));
    } else if VIDEO.contains(&t) {
        // An animation into a video: even sides, a pixel format players accept.
        let mut a = v(&["-movflags", "+faststart"]);
        a.extend(scaled(encode(t, quality), width));
        tries.push(a);
    } else if ANIMATED.contains(&t) && VIDEO.contains(&f) {
        let fps = step.options.fps.unwrap_or(12).clamp(1, 50);
        let w = width.unwrap_or(640);
        let filters = format!("fps={fps},scale='min({w},iw)':-2:flags=lanczos");
        tries.push(match t {
            "gif" => vec![
                "-vf".into(),
                format!("{filters},split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle"),
                "-loop".into(),
                "0".into(),
            ],
            "webp" => vec!["-vf".into(), filters, "-c:v".into(), "libwebp_anim".into(), "-loop".into(), "0".into(), "-q:v".into(), "75".into(), "-an".into()],
            _ => vec!["-vf".into(), filters, "-plays".into(), "0".into(), "-an".into()],
        });
    } else if VIDEO.contains(&f) {
        // One picture: FFmpeg's thumbnail filter picks a representative frame.
        tries.push(v(&[
            "-vf",
            "thumbnail=150",
            "-frames:v",
            "1",
            "-update",
            "1",
        ]));
    } else if SUBTITLES.contains(&t) {
        tries.push(Vec::new());
    } else {
        // A still image.
        let mut a = v(&["-frames:v", "1", "-update", "1"]);
        match t {
            "avif" => a.extend(v(&[
                "-c:v",
                "libaom-av1",
                "-still-picture",
                "1",
                "-crf",
                "30",
                "-pix_fmt",
                "yuv420p",
            ])),
            "jxl" => a.extend(v(&["-c:v", "libjxl", "-distance", "1"])),
            "jpg" => a.extend(v(&["-q:v", "2"])),
            _ => {}
        }
        tries.push(a);
    }

    let duration = probe_duration(&ffmpeg, input, &mut step.cancel).await;
    let mut last = Error::Engine("FFmpeg could not convert this".into());
    for args in tries {
        let mut cmd = run::command(&ffmpeg);
        cmd.args([
            "-hide_banner",
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-progress",
            "pipe:1",
            "-nostats",
            "-i",
        ])
        .arg(input)
        .args(&args);
        if let Some(m) = muxer(t) {
            if !args.iter().any(|a| a == "-f") {
                cmd.args(["-f", m]);
            }
        }
        cmd.arg(output);
        let report = step.report;
        let progress = move |line: &str| {
            if let (Some(us), Some(total)) = (
                line.strip_prefix("out_time_us=")
                    .and_then(|v| v.parse::<f64>().ok()),
                duration,
            ) {
                if total > 0.0 {
                    report(((us / 1e6) / total).clamp(0.0, 1.0) as f32);
                }
            }
        };
        match run::run(cmd, &mut step.cancel, &progress).await {
            Ok(()) if std::fs::metadata(output).is_ok_and(|m| m.len() > 0) => return Ok(()),
            Ok(()) => last = Error::Engine("FFmpeg wrote an empty file".into()),
            Err(Error::Cancelled) => return Err(Error::Cancelled),
            Err(e) => last = e,
        }
        let _ = std::fs::remove_file(output);
    }
    Err(last)
}

/// Add a width limit to an encode's video filter.
fn scaled(mut args: Vec<String>, width: Option<u32>) -> Vec<String> {
    if let Some(w) = width {
        let limit = format!("scale='min({w},iw)':-2");
        if let Some(i) = args.iter().position(|a| a == "-vf") {
            if let Some(f) = args.get_mut(i + 1) {
                *f = format!("{limit},{f}");
            }
        } else {
            args.extend(["-vf".into(), limit]);
        }
    }
    args
}

/// The input's length in seconds, for progress.
async fn probe_duration(
    ffmpeg: &Path,
    input: &Path,
    cancel: &mut tokio::sync::watch::Receiver<bool>,
) -> Option<f64> {
    let found = std::sync::Mutex::new(None);
    let mut cmd = run::command(ffmpeg);
    cmd.args(["-hide_banner", "-nostdin", "-i"]).arg(input);
    let _ = run::run(cmd, cancel, &|line: &str| {
        if let Some(rest) = line.trim().strip_prefix("Duration: ") {
            let stamp = rest.split(',').next().unwrap_or_default();
            let parts: Vec<f64> = stamp.split(':').filter_map(|p| p.parse().ok()).collect();
            if let [h, m, s] = parts[..] {
                if let Ok(mut f) = found.lock() {
                    *f = Some(h * 3600.0 + m * 60.0 + s);
                }
            }
        }
    })
    .await;
    found.into_inner().ok().flatten()
}
