// SPDX-License-Identifier: GPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 EON contributors
//
// Additional permission under GNU GPL version 3 section 7:
// see LICENSE-EXCEPTION.md (EON Module ABI Exception 1.0).

//! Every format the converter knows: what it is called, which family it belongs
//! to, and how it is recognised. A format's `id` is its identity everywhere: in
//! the routing graph, in the application, and in EON Download Manager's choices.

use serde::Serialize;
use std::path::Path;

/// What the application groups formats by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Video,
    Audio,
    Image,
    Document,
    Spreadsheet,
    Presentation,
    Ebook,
    Data,
    Subtitle,
    Archive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Format {
    pub id: &'static str,
    pub name: &'static str,
    pub family: Family,
    /// The first is what a converted file is given.
    pub extensions: &'static [&'static str],
    pub mime: &'static [&'static str],
}

macro_rules! formats {
    ($( $id:literal, $name:literal, $family:ident, [$($ext:literal),*], [$($mime:literal),*]; )*) => {
        /// The whole table. Its order is the order the application lists formats in.
        pub const FORMATS: &[Format] = &[
            $( Format { id: $id, name: $name, family: Family::$family, extensions: &[$($ext),*], mime: &[$($mime),*] }, )*
        ];
    };
}

formats! {
    // ---- video ------------------------------------------------------------------
    "mp4",  "MP4",            Video, ["mp4"], ["video/mp4"];
    "mkv",  "Matroska (MKV)", Video, ["mkv"], ["video/x-matroska"];
    "webm", "WebM",           Video, ["webm"], ["video/webm"];
    "mov",  "QuickTime (MOV)", Video, ["mov", "qt"], ["video/quicktime"];
    "avi",  "AVI",            Video, ["avi"], ["video/x-msvideo"];
    "wmv",  "Windows Media Video", Video, ["wmv"], ["video/x-ms-wmv"];
    "flv",  "Flash Video",    Video, ["flv"], ["video/x-flv"];
    "mpeg", "MPEG",           Video, ["mpg", "mpeg"], ["video/mpeg"];
    "ts",   "MPEG-TS",        Video, ["ts", "m2ts", "mts"], ["video/mp2t"];
    "m4v",  "M4V",            Video, ["m4v"], ["video/x-m4v"];
    "3gp",  "3GP",            Video, ["3gp"], ["video/3gpp"];
    "ogv",  "Ogg Video",      Video, ["ogv"], ["video/ogg"];
    // ---- audio ------------------------------------------------------------------
    "mp3",  "MP3",            Audio, ["mp3"], ["audio/mpeg"];
    "aac",  "AAC",            Audio, ["aac"], ["audio/aac"];
    "m4a",  "M4A (AAC)",      Audio, ["m4a"], ["audio/mp4"];
    "wav",  "WAV",            Audio, ["wav"], ["audio/wav", "audio/x-wav"];
    "flac", "FLAC",           Audio, ["flac"], ["audio/flac"];
    "ogg",  "Ogg Vorbis",     Audio, ["ogg", "oga"], ["audio/ogg"];
    "opus", "Opus",           Audio, ["opus"], ["audio/opus"];
    "wma",  "Windows Media Audio", Audio, ["wma"], ["audio/x-ms-wma"];
    "aiff", "AIFF",           Audio, ["aiff", "aif"], ["audio/aiff"];
    "ac3",  "AC-3",           Audio, ["ac3"], ["audio/ac3"];
    "alac", "Apple Lossless (M4A)", Audio, ["m4a"], [];
    "amr",  "AMR",            Audio, ["amr"], ["audio/amr"];
    "mka",  "Matroska Audio", Audio, ["mka"], ["audio/x-matroska"];
    // ---- image (animated ones too) ----------------------------------------------------
    "png",  "PNG",            Image, ["png"], ["image/png"];
    "jpg",  "JPEG",           Image, ["jpg", "jpeg", "jpe", "jfif"], ["image/jpeg"];
    "webp", "WebP",           Image, ["webp"], ["image/webp"];
    "gif",  "GIF",            Image, ["gif"], ["image/gif"];
    "apng", "Animated PNG",   Image, ["apng"], ["image/apng"];
    "bmp",  "BMP",            Image, ["bmp"], ["image/bmp"];
    "tiff", "TIFF",           Image, ["tif", "tiff"], ["image/tiff"];
    "ico",  "Icon (ICO)",     Image, ["ico"], ["image/x-icon", "image/vnd.microsoft.icon"];
    "avif", "AVIF",           Image, ["avif"], ["image/avif"];
    "heic", "HEIC / HEIF",    Image, ["heic", "heif"], ["image/heic", "image/heif"];
    "jxl",  "JPEG XL",        Image, ["jxl"], ["image/jxl"];
    "jp2",  "JPEG 2000",      Image, ["jp2", "j2k"], ["image/jp2"];
    "tga",  "TGA",            Image, ["tga"], ["image/x-tga"];
    "qoi",  "QOI",            Image, ["qoi"], [];
    "pnm",  "PNM / PPM",      Image, ["ppm", "pnm", "pgm", "pbm"], ["image/x-portable-anymap"];
    "hdr",  "Radiance HDR",   Image, ["hdr"], [];
    "dds",  "DirectDraw Surface", Image, ["dds"], [];
    "psd",  "Photoshop (PSD)", Image, ["psd"], ["image/vnd.adobe.photoshop"];
    "svg",  "SVG",            Image, ["svg"], ["image/svg+xml"];
    "pcx",  "PCX",            Image, ["pcx"], ["image/x-pcx"];
    "xcf",  "GIMP (XCF)",     Image, ["xcf"], [];
    // ---- documents ----------------------------------------------------------------
    "pdf",  "PDF",            Document, ["pdf"], ["application/pdf"];
    "txt",  "Plain text",     Document, ["txt", "text"], ["text/plain"];
    "md",   "Markdown",       Document, ["md", "markdown"], ["text/markdown"];
    "gfm",  "GitHub Markdown", Document, ["md"], [];
    "html", "HTML",           Document, ["html", "htm", "xhtml"], ["text/html"];
    "docx", "Word (DOCX)",    Document, ["docx"], ["application/vnd.openxmlformats-officedocument.wordprocessingml.document"];
    "doc",  "Word 97 (DOC)",  Document, ["doc"], ["application/msword"];
    "odt",  "OpenDocument Text", Document, ["odt"], ["application/vnd.oasis.opendocument.text"];
    "rtf",  "Rich Text (RTF)", Document, ["rtf"], ["application/rtf", "text/rtf"];
    "tex",  "LaTeX",          Document, ["tex", "latex"], ["application/x-tex"];
    "rst",  "reStructuredText", Document, ["rst"], ["text/x-rst"];
    "org",  "Org",            Document, ["org"], [];
    "adoc", "AsciiDoc",       Document, ["adoc", "asciidoc"], [];
    "textile", "Textile",     Document, ["textile"], [];
    "mediawiki", "MediaWiki", Document, ["wiki", "mediawiki"], [];
    "dokuwiki", "DokuWiki",   Document, ["dokuwiki"], [];
    "jira", "Jira wiki",      Document, ["jira"], [];
    "docbook", "DocBook",     Document, ["dbk", "docbook"], ["application/docbook+xml"];
    "ipynb", "Jupyter Notebook", Document, ["ipynb"], ["application/x-ipynb+json"];
    "typst", "Typst",         Document, ["typ"], [];
    "man",  "Manual page",    Document, ["man", "1"], ["text/troff"];
    // ---- spreadsheets ---------------------------------------------------------------
    "xlsx", "Excel (XLSX)",   Spreadsheet, ["xlsx"], ["application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"];
    "xls",  "Excel 97 (XLS)", Spreadsheet, ["xls"], ["application/vnd.ms-excel"];
    "xlsb", "Excel binary (XLSB)", Spreadsheet, ["xlsb"], ["application/vnd.ms-excel.sheet.binary.macroenabled.12"];
    "ods",  "OpenDocument Spreadsheet", Spreadsheet, ["ods"], ["application/vnd.oasis.opendocument.spreadsheet"];
    "csv",  "CSV",            Spreadsheet, ["csv"], ["text/csv"];
    "tsv",  "TSV",            Spreadsheet, ["tsv", "tab"], ["text/tab-separated-values"];
    // ---- presentations ------------------------------------------------------------------
    "pptx", "PowerPoint (PPTX)", Presentation, ["pptx"], ["application/vnd.openxmlformats-officedocument.presentationml.presentation"];
    "ppt",  "PowerPoint 97 (PPT)", Presentation, ["ppt"], ["application/vnd.ms-powerpoint"];
    "odp",  "OpenDocument Presentation", Presentation, ["odp"], ["application/vnd.oasis.opendocument.presentation"];
    // ---- ebooks -----------------------------------------------------------------------
    "epub", "EPUB",           Ebook, ["epub"], ["application/epub+zip"];
    "fb2",  "FictionBook (FB2)", Ebook, ["fb2"], ["application/x-fictionbook+xml"];
    "mobi", "Mobipocket (MOBI)", Ebook, ["mobi"], ["application/x-mobipocket-ebook"];
    "azw3", "Kindle (AZW3)",  Ebook, ["azw3"], ["application/vnd.amazon.ebook"];
    // ---- data -------------------------------------------------------------------------
    "json", "JSON",           Data, ["json"], ["application/json"];
    "yaml", "YAML",           Data, ["yaml", "yml"], ["application/yaml", "text/yaml"];
    "toml", "TOML",           Data, ["toml"], ["application/toml"];
    "xml",  "XML",            Data, ["xml"], ["application/xml", "text/xml"];
    // ---- subtitles ----------------------------------------------------------------------
    "srt",  "SubRip (SRT)",   Subtitle, ["srt"], ["application/x-subrip"];
    "vtt",  "WebVTT",         Subtitle, ["vtt"], ["text/vtt"];
    "ass",  "Advanced SubStation (ASS)", Subtitle, ["ass", "ssa"], ["text/x-ass"];
    // ---- archives -----------------------------------------------------------------------
    "zip",  "ZIP",            Archive, ["zip"], ["application/zip"];
    "tar",  "TAR",            Archive, ["tar"], ["application/x-tar"];
    "tgz",  "TAR.GZ",         Archive, ["tar.gz", "tgz"], ["application/gzip"];
}

/// The format with this id.
pub fn by_id(id: &str) -> Option<&'static Format> {
    FORMATS.iter().find(|f| f.id == id)
}

/// The first format that claims this extension (`"JPEG"`, `".jpg"` and `"jpg"` alike).
pub fn by_extension(ext: &str) -> Option<&'static Format> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    FORMATS
        .iter()
        .find(|f| f.extensions.contains(&ext.as_str()))
}

/// The first format that claims this MIME type (parameters ignored).
pub fn by_mime(mime: &str) -> Option<&'static Format> {
    let mime = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    FORMATS.iter().find(|f| f.mime.contains(&mime.as_str()))
}

/// The extension a file name carries, `tar.gz` included.
pub fn extension_of(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") {
        return Some("tar.gz".into());
    }
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

/// What `bytes` (a file's start) and its name say it is. The content wins when
/// it is unambiguous; a container that holds many kinds (ZIP holds DOCX, XLSX,
/// EPUB...) defers to the name.
pub fn detect(bytes: &[u8], path: &Path) -> Option<&'static Format> {
    let by_name = extension_of(path).and_then(|e| by_extension(&e));
    let sniffed = infer::get(bytes)
        .and_then(|k| by_mime(k.mime_type()).or_else(|| by_extension(k.extension())));
    match (sniffed, by_name) {
        // A ZIP whose name says DOCX, EPUB, XLSX, ODT...: trust the name.
        (Some(s), Some(n)) if s.id == "zip" && n.family != Family::Archive => Some(n),
        // An MP4 container named M4A, M4V or MOV: the name is more precise.
        (Some(s), Some(n)) if s.id == "mp4" && ["m4a", "m4v", "mov", "3gp"].contains(&n.id) => {
            Some(n)
        }
        // MKV and WebM share a container; so do OGG audio and video.
        (Some(s), Some(n)) if s.id == "mkv" && ["webm", "mka"].contains(&n.id) => Some(n),
        (Some(s), Some(n)) if s.id == "ogg" && ["ogv", "opus"].contains(&n.id) => Some(n),
        // WMV and WMA are both ASF; the content cannot tell them apart.
        (Some(s), Some(n)) if s.id == "wmv" && n.id == "wma" => Some(n),
        // A gzip named .tar.gz is a TAR.GZ.
        (Some(s), Some(n)) if s.id == "tgz" && n.id == "tgz" => Some(n),
        (Some(s), _) => Some(s),
        (None, n) => n,
    }
}

/// Whether a file detected as `got` is a `want`. A format told only by its
/// name agrees with any row sharing its extensions; one container may stand for
/// another of its family under the same extension.
pub fn agrees(got: &Format, want: &Format) -> bool {
    got.id == want.id
        || got.extensions == want.extensions
        || (got.family == want.family && got.extensions[0] == want.extensions[0])
}

/// Read the start of a file and say what it is.
pub fn detect_file(path: &Path) -> std::io::Result<Option<&'static Format>> {
    use std::io::Read;
    let mut head = vec![0u8; 8192];
    let n = std::fs::File::open(path)?.read(&mut head)?;
    head.truncate(n);
    Ok(detect(&head, path))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_every_format_has_an_extension() {
        let mut ids: Vec<_> = FORMATS.iter().map(|f| f.id).collect();
        ids.sort_unstable();
        let n = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate ids");
        assert!(FORMATS.iter().all(|f| !f.extensions.is_empty()));
    }

    #[test]
    fn content_beats_a_wrong_name_but_a_zip_trusts_its_name() {
        let png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR";
        assert_eq!(detect(png, Path::new("photo.jpg")).unwrap().id, "png");
        let zip = b"PK\x03\x04\x14\0\0\0\0\0";
        assert_eq!(detect(zip, Path::new("report.docx")).unwrap().id, "docx");
        assert_eq!(detect(zip, Path::new("bundle.zip")).unwrap().id, "zip");
        assert_eq!(
            detect(b"# Title\n", Path::new("notes.md")).unwrap().id,
            "md"
        );
        assert_eq!(detect(b"", Path::new("a.tar.gz")).unwrap().id, "tgz");
        assert_eq!(by_extension(".JPEG").unwrap().id, "jpg");
        assert_eq!(by_mime("text/html; charset=utf-8").unwrap().id, "html");
    }

    #[test]
    fn an_asf_container_named_wma_is_audio() {
        // The ASF header GUID: WMV and WMA share it, so the name decides.
        let asf = b"\x30\x26\xb2\x75\x8e\x66\xcf\x11\xa6\xd9\x00\xaa\x00\x62\xce\x6c";
        assert_eq!(detect(asf, Path::new("song.wma")).unwrap().id, "wma");
        assert_eq!(detect(asf, Path::new("clip.wmv")).unwrap().id, "wmv");
    }
}
