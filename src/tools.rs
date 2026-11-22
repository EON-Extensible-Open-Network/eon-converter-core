// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! The engines: fetched the first time a conversion needs one, never bundled.
//!
//! Each comes from its own project's release page at a pinned version, and is
//! checked against the SHA-256 pinned here (madde 37). A file that does not
//! match is deleted and never run. Everything lives in `<data>/tools/<engine>/`.
//! LibreOffice and Calibre need an administrator install, so they are never
//! fetched: they are used only when they are already on the system.

use crate::Error;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The engines the core can run. `Core` is the core itself: nothing to fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    Core,
    Ffmpeg,
    Pandoc,
    Typst,
    Pdfium,
    Magick,
    LibreOffice,
    Calibre,
}

impl Engine {
    pub fn name(self) -> &'static str {
        match self {
            Engine::Core => "EON Converter",
            Engine::Ffmpeg => "FFmpeg",
            Engine::Pandoc => "Pandoc",
            Engine::Typst => "Typst",
            Engine::Pdfium => "PDFium",
            Engine::Magick => "ImageMagick",
            Engine::LibreOffice => "LibreOffice",
            Engine::Calibre => "Calibre",
        }
    }

    /// Fetched on demand (as opposed to the core itself, or installed by the person).
    pub fn fetched(self) -> bool {
        !matches!(self, Engine::Core | Engine::LibreOffice | Engine::Calibre)
    }
}

/// One downloadable build of an engine.
struct Pin {
    url: &'static str,
    sha256: &'static str,
    /// Approximate download size, shown before fetching.
    bytes: u64,
    /// The program (or library) inside the archive, by file name.
    inside: &'static str,
}

/// The pinned build for this system, if the project publishes one.
fn pin(engine: Engine) -> Option<Pin> {
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let p = |url, sha256, bytes, inside| {
        Some(Pin {
            url,
            sha256,
            bytes,
            inside,
        })
    };
    match (engine, os, arch) {
        // FFmpeg: BtbN's static GPL builds, a dated build rather than the moving "latest".
        (Engine::Ffmpeg, "windows", "x86_64") => p(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-21-12-37/ffmpeg-n8.1.3-39-g660b5aa7db-win64-gpl-8.1.zip",
            "bac21c7550d0daee47ab58f19e93d285fdb040c95252e63d18c36978a131f1ca", 197_396_675, "ffmpeg.exe"),
        (Engine::Ffmpeg, "windows", "aarch64") => p(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-21-12-37/ffmpeg-n8.1.3-39-g660b5aa7db-winarm64-gpl-8.1.zip",
            "031c5fa988c7f5bed8b9dd6c807f03e626133bcb15775553cc785ceffd93c712", 137_859_255, "ffmpeg.exe"),
        (Engine::Ffmpeg, "linux", "x86_64") => p(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-21-12-37/ffmpeg-n8.1.3-39-g660b5aa7db-linux64-gpl-8.1.tar.xz",
            "85d6d2c617fa4e2bee9ea105b5532c66ac866ada0a9a650a088e6fb096c7eeff", 152_916_112, "ffmpeg"),
        (Engine::Ffmpeg, "linux", "aarch64") => p(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-21-12-37/ffmpeg-n8.1.3-39-g660b5aa7db-linuxarm64-gpl-8.1.tar.xz",
            "d221332b645f15bdef4900948ed509417d68e3f4c07fd6453426c8aa3e99a0ea", 128_376_536, "ffmpeg"),
        // Pandoc 3.14.
        (Engine::Pandoc, "windows", _) => p(
            "https://github.com/jgm/pandoc/releases/download/3.14/pandoc-3.14-windows-x86_64.zip",
            "0144b96e8c7b023fd382ec5ef5bdc475e873f1019d6fa72502492df2c9a94a3b", 43_614_099, "pandoc.exe"),
        (Engine::Pandoc, "linux", "x86_64") => p(
            "https://github.com/jgm/pandoc/releases/download/3.14/pandoc-3.14-linux-amd64.tar.gz",
            "c5f4720cdefb3f7bdc5d07202ef579feef0d1370773c873804dd8fdaa8c4f1b8", 36_635_549, "pandoc"),
        (Engine::Pandoc, "linux", "aarch64") => p(
            "https://github.com/jgm/pandoc/releases/download/3.14/pandoc-3.14-linux-arm64.tar.gz",
            "c692f3d78a415741bbbac4cdbbbe26fa7edca1f43d2301b982fbf4009cc8cf7f", 39_208_196, "pandoc"),
        (Engine::Pandoc, "macos", "aarch64") => p(
            "https://github.com/jgm/pandoc/releases/download/3.14/pandoc-3.14-arm64-macOS.zip",
            "58d6593884fec9d4424b641b92503c316d9c36decb2891ff8d73ae74dd3b6304", 43_712_865, "pandoc"),
        (Engine::Pandoc, "macos", "x86_64") => p(
            "https://github.com/jgm/pandoc/releases/download/3.14/pandoc-3.14-x86_64-macOS.zip",
            "891708579061e7b1c1a6ce0df30ebb3cb2b6d0ca045ed3c8554ab830c1e0c270", 27_490_795, "pandoc"),
        // Typst 0.15.1.
        (Engine::Typst, "windows", "x86_64") => p(
            "https://github.com/typst/typst/releases/download/v0.15.1/typst-x86_64-pc-windows-msvc.zip",
            "19ce3551153c2fe7ee9fa2f95208310c8f4d3209fedb699e0333faf8913f6736", 22_463_684, "typst.exe"),
        (Engine::Typst, "windows", "aarch64") => p(
            "https://github.com/typst/typst/releases/download/v0.15.1/typst-aarch64-pc-windows-msvc.zip",
            "4ab28e1b71ec3184d38d580ab797f499b6770d952b6b19167be5cea5c2662e14", 21_246_627, "typst.exe"),
        (Engine::Typst, "linux", "x86_64") => p(
            "https://github.com/typst/typst/releases/download/v0.15.1/typst-x86_64-unknown-linux-musl.tar.xz",
            "a6d077d0a95eed5a2eba715b2dae06be954f624ccbf85758a03f389ded33118c", 17_462_992, "typst"),
        (Engine::Typst, "linux", "aarch64") => p(
            "https://github.com/typst/typst/releases/download/v0.15.1/typst-aarch64-unknown-linux-musl.tar.xz",
            "5aa8d74a3d906e60ea12a66ac2f37f8eef1b14cbad7182a745e393a10c23dcee", 16_216_812, "typst"),
        (Engine::Typst, "macos", "aarch64") => p(
            "https://github.com/typst/typst/releases/download/v0.15.1/typst-aarch64-apple-darwin.tar.xz",
            "48f62ed034aa3a7978309579ac6ca00045e2ef0da73114e8af27cfd8e74dc05a", 14_438_168, "typst"),
        (Engine::Typst, "macos", "x86_64") => p(
            "https://github.com/typst/typst/releases/download/v0.15.1/typst-x86_64-apple-darwin.tar.xz",
            "7f9fdd9584866245de9a79e0add8f9236fae6f40a8a45e2c4771ccc14db4e0fa", 15_635_600, "typst"),
        // PDFium, chromium/8165, as built by bblanchon.
        (Engine::Pdfium, "windows", "x86_64") => p(
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8165/pdfium-win-x64.tgz",
            "ca7338ae7779b8b1b82f4580305c4d762d214f8df1d54b47647fe3cc75989ded", 4_000_000, "pdfium.dll"),
        (Engine::Pdfium, "windows", "aarch64") => p(
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8165/pdfium-win-arm64.tgz",
            "d65e405c59cffe6fe36c0935b90d40ddf3ce75ef284f3168a28dcf00d0388131", 4_000_000, "pdfium.dll"),
        (Engine::Pdfium, "linux", "x86_64") => p(
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8165/pdfium-linux-x64.tgz",
            "ba7865e16c0271daab75256790be06c26272190afe6e2a70eae0bb39bb99089c", 4_000_000, "libpdfium.so"),
        (Engine::Pdfium, "linux", "aarch64") => p(
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8165/pdfium-linux-arm64.tgz",
            "338f5dcc907a20331812ae8ab01b16ff18966f4a247d790c8ad8fb49029d768b", 3_817_479, "libpdfium.so"),
        (Engine::Pdfium, "macos", "aarch64") => p(
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8165/pdfium-mac-arm64.tgz",
            "112e74d717f3e0310273b70f3c8780e8a61ff11b34834124465c8305c841d350", 4_000_000, "libpdfium.dylib"),
        (Engine::Pdfium, "macos", "x86_64") => p(
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8165/pdfium-mac-x64.tgz",
            "07285915b564e46a432a9909dbc1f15892885ca6489e3faeefc70dee77ed371a", 4_000_000, "libpdfium.dylib"),
        // ImageMagick 7.1.2-36, portable builds.
        (Engine::Magick, "windows", "x86_64") => p(
            "https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-36/ImageMagick-7.1.2-36-portable-Q16-HDRI-x64.7z",
            "e7c705840e2d989eb9fdbd5c2ac87c5b7a9f95b97daee1a1e6e66aa152548ae5", 13_000_000, "magick.exe"),
        (Engine::Magick, "windows", "aarch64") => p(
            "https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-36/ImageMagick-7.1.2-36-portable-Q16-HDRI-arm64.7z",
            "736182aefac4924b327c049885d93c8a971d43205a470e98610fd8a1ff96b3f2", 12_899_155, "magick.exe"),
        (Engine::Magick, "linux", "x86_64") => p(
            "https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-36/ImageMagick-7.1.2-36-portable-Q16-HDRI-linux-x64.tar.xz",
            "72c8dd0268f824f621e1628737b79d8e80bc63a5d46a69950dabb13c5de80c91", 12_869_884, "magick"),
        (Engine::Magick, "linux", "aarch64") => p(
            "https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-36/ImageMagick-7.1.2-36-portable-Q16-HDRI-linux-arm64.tar.xz",
            "456146795540708e3fe67af4847f3aeb23c614593572f8d65a50b9bb663ee2db", 11_705_736, "magick"),
        (Engine::Magick, "macos", "aarch64") => p(
            "https://github.com/ImageMagick/ImageMagick/releases/download/7.1.2-36/ImageMagick-7.1.2-36-portable-Q16-HDRI-macos-arm64.tar.xz",
            "224157488f2863a834c979607fe6666d749075279c12a2d01c41917d4acd561e", 9_007_560, "magick"),
        _ => None,
    }
}

fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(exe(name)))
        .find(|p| p.is_file())
}

/// Where the engines live.
#[derive(Debug, Clone)]
pub struct Tools {
    dir: PathBuf,
}

/// What the application shows about an engine.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub engine: Engine,
    pub name: &'static str,
    /// Ready to run now.
    pub ready: bool,
    /// Can be fetched (a build is pinned for this system).
    pub fetchable: bool,
    pub bytes: Option<u64>,
}

impl Tools {
    pub fn new(data_dir: &Path) -> Self {
        Tools {
            dir: data_dir.join("tools"),
        }
    }

    fn home(&self, engine: Engine) -> PathBuf {
        self.dir.join(format!("{engine:?}").to_ascii_lowercase())
    }

    /// The engine's program (or library) if it is here: our own copy first,
    /// then one the system already has.
    pub fn find(&self, engine: Engine) -> Option<PathBuf> {
        if let Some(pin) = pin(engine) {
            let own = self.home(engine).join(pin.inside);
            if own.is_file() {
                return Some(own);
            }
        }
        match engine {
            Engine::Core | Engine::Pdfium => None,
            Engine::Ffmpeg => on_path("ffmpeg"),
            Engine::Pandoc => on_path("pandoc"),
            Engine::Typst => on_path("typst"),
            Engine::Magick => on_path("magick"),
            Engine::LibreOffice => libreoffice(),
            Engine::Calibre => on_path("ebook-convert").or_else(|| {
                [
                    r"C:\Program Files\Calibre2\ebook-convert.exe",
                    "/Applications/calibre.app/Contents/MacOS/ebook-convert",
                ]
                .iter()
                .map(PathBuf::from)
                .find(|p| p.is_file())
            }),
        }
    }

    pub fn status(&self, engine: Engine) -> Status {
        let pin = pin(engine);
        Status {
            engine,
            name: engine.name(),
            ready: engine == Engine::Core || self.find(engine).is_some(),
            fetchable: pin.is_some(),
            bytes: pin.map(|p| p.bytes),
        }
    }

    /// The engine, fetched and checked if it is not here yet.
    pub async fn ensure(&self, client: &reqwest::Client, engine: Engine) -> Result<PathBuf, Error> {
        if let Some(found) = self.find(engine) {
            return Ok(found);
        }
        let pin = pin(engine).ok_or_else(|| {
            Error::Missing(format!(
                "{} is needed for this and is not on this system{}",
                engine.name(),
                if engine.fetched() {
                    "; no build is published for it here"
                } else {
                    "; install it to enable this"
                }
            ))
        })?;
        let home = self.home(engine);
        let work = self
            .dir
            .join(format!("{}.unpack", engine.name().to_ascii_lowercase()));
        let _ = tokio::fs::remove_dir_all(&work).await;
        tokio::fs::create_dir_all(&work).await.map_err(io)?;
        let archive_name = pin.url.rsplit('/').next().unwrap_or("archive");
        let archive = work.join(archive_name);
        download(client, pin.url, pin.sha256, &archive).await?;
        unpack(&archive, &work).await?;
        let _ = tokio::fs::remove_file(&archive).await;
        // The program's folder becomes the engine's home, with everything beside it
        // (ImageMagick and FFmpeg keep their libraries and data next to the program).
        let found = find_file(&work, pin.inside)
            .ok_or_else(|| Error::Invalid(format!("{} is not in its archive", pin.inside)))?;
        let folder = found
            .parent()
            .map_or_else(|| work.clone(), Path::to_path_buf);
        let _ = tokio::fs::remove_dir_all(&home).await;
        tokio::fs::rename(&folder, &home).await.map_err(io)?;
        let _ = tokio::fs::remove_dir_all(&work).await;
        let path = home.join(pin.inside);
        make_executable(&path);
        Ok(path)
    }
}

/// LibreOffice's command-line program, where installers put it.
fn libreoffice() -> Option<PathBuf> {
    on_path("soffice").or_else(|| {
        [
            r"C:\Program Files\LibreOffice\program\soffice.exe",
            r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
            "/Applications/LibreOffice.app/Contents/MacOS/soffice",
            "/usr/bin/libreoffice",
            "/usr/lib/libreoffice/program/soffice",
            "/opt/libreoffice/program/soffice",
        ]
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
    })
}

#[allow(clippy::needless_pass_by_value)] // the shape `map_err` calls
fn io(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Stream `url` into `path`, hashing as it goes. Kept only if the SHA-256 is the
/// pinned one.
async fn download(
    client: &reqwest::Client,
    url: &str,
    sha256: &str,
    path: &Path,
) -> Result<(), Error> {
    use tokio::io::AsyncWriteExt;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(Error::Network(format!("{url}: {}", response.status())));
    }
    let mut file = tokio::fs::File::create(path).await.map_err(io)?;
    let mut hasher = Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| Error::Network(e.to_string()))?
    {
        hasher.update(&chunk);
        file.write_all(&chunk).await.map_err(io)?;
    }
    file.flush().await.map_err(io)?;
    drop(file);
    if hex(&hasher.finalize()) == sha256 {
        Ok(())
    } else {
        let _ = tokio::fs::remove_file(path).await;
        Err(Error::Invalid(format!(
            "{url} does not match its pinned SHA-256; not used"
        )))
    }
}

/// The system's own `tar` (bsdtar on Windows and macOS reads ZIP, 7z, gzip and
/// xz; GNU tar on Linux reads gzip and xz).
fn system_tar() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("SystemRoot")
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from)
            .join("System32")
            .join("tar.exe")
    } else {
        PathBuf::from("tar")
    }
}

async fn unpack(archive: &Path, into: &Path) -> Result<(), Error> {
    let out = crate::run::command(&system_tar())
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .output()
        .await
        .map_err(io)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "{} could not be unpacked: {}",
            archive.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        )))
    }
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).ok()?.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.file_name().is_some_and(|n| n == name) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}

#[cfg(not(unix))]
fn make_executable(_: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pin_is_a_full_sha256_over_https() {
        for engine in [
            Engine::Ffmpeg,
            Engine::Pandoc,
            Engine::Typst,
            Engine::Pdfium,
            Engine::Magick,
        ] {
            if let Some(p) = pin(engine) {
                assert!(p.url.starts_with("https://"));
                assert_eq!(p.sha256.len(), 64);
                assert!(p
                    .sha256
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            }
        }
    }
}
