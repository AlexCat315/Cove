//! Content types inferred from a filename.
//!
//! A drop carries a path, not a MIME type, and the platform does not always
//! report one. This table is the fallback: an extension maps to the type a
//! server expects, and anything unknown becomes `application/octet-stream` so a
//! request is always well-formed. It is a lookup, not sniffing — the bytes are
//! never read here, because a zone must describe a drop before anything is read.

/// The `Content-Type` for `file_name`.
///
/// The match is case-insensitive and ignores a leading dot. A name with no
/// extension (or a hidden file such as `.gitignore`) is octet-stream.
pub fn content_type_for(file_name: &str) -> &'static str {
    match extension_of(file_name)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") | Some("jpe") | Some("jfif") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("bmp") => "image/bmp",
        Some("tif") | Some("tiff") => "image/tiff",
        Some("svg") => "image/svg+xml",
        Some("heic") => "image/heic",
        Some("heif") => "image/heif",
        Some("ico") => "image/x-icon",

        Some("pdf") => "application/pdf",
        Some("json") => "application/json",
        Some("xml") => "application/xml",
        Some("yaml") | Some("yml") => "application/yaml",
        Some("toml") => "application/toml",
        Some("csv") => "text/csv",
        Some("html") | Some("htm") => "text/html",
        Some("md") | Some("markdown") => "text/markdown",
        Some("txt") | Some("text") | Some("log") => "text/plain",
        Some("rtf") => "application/rtf",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("xls") => "application/vnd.ms-excel",
        Some("xlsx") => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        Some("ppt") => "application/vnd.ms-powerpoint",
        Some("pptx") => "application/vnd.openxmlformats-officedocument.presentationml.presentation",

        Some("zip") => "application/zip",
        Some("gz") => "application/gzip",
        Some("tar") => "application/x-tar",
        Some("bz2") => "application/x-bzip2",
        Some("xz") => "application/x-xz",
        Some("7z") => "application/x-7z-compressed",
        Some("rar") => "application/vnd.rar",
        Some("zst") => "application/zstd",

        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("aac") => "audio/aac",
        Some("ogg") | Some("oga") => "audio/ogg",
        Some("m4a") => "audio/mp4",
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",

        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",

        _ => "application/octet-stream",
    }
}

/// Whether `file_name` is a known type rather than the octet-stream fallback.
pub fn is_known(file_name: &str) -> bool {
    content_type_for(file_name) != "application/octet-stream"
}

/// The lowercase extension without its dot.
///
/// A leading dot marks a hidden file, not an extension.
fn extension_of(file_name: &str) -> Option<&str> {
    let name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let (stem, extension) = name.rsplit_once('.')?;
    if stem.is_empty() || extension.is_empty() {
        return None;
    }
    Some(extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_map_to_their_types() {
        assert_eq!(content_type_for("a.png"), "image/png");
        assert_eq!(content_type_for("A.JPEG"), "image/jpeg");
        assert_eq!(content_type_for("b.webp"), "image/webp");
        assert_eq!(content_type_for("c.svg"), "image/svg+xml");
    }

    #[test]
    fn archives_are_recognised_not_rejected() {
        assert_eq!(content_type_for("pack.zip"), "application/zip");
        assert_eq!(content_type_for("a.tar.gz"), "application/gzip");
        assert_eq!(content_type_for("b.7z"), "application/x-7z-compressed");
    }

    #[test]
    fn text_and_documents_are_distinguished() {
        assert_eq!(content_type_for("notes.txt"), "text/plain");
        assert_eq!(content_type_for("data.csv"), "text/csv");
        assert_eq!(content_type_for("report.pdf"), "application/pdf");
        assert_eq!(content_type_for("doc.md"), "text/markdown");
    }

    #[test]
    fn an_unknown_or_missing_extension_is_octet_stream() {
        assert_eq!(content_type_for("a.unknownext"), "application/octet-stream");
        assert_eq!(content_type_for("README"), "application/octet-stream");
        assert_eq!(content_type_for(".gitignore"), "application/octet-stream");
        assert_eq!(content_type_for("archive."), "application/octet-stream");
    }

    #[test]
    fn a_path_is_reduced_to_its_name_first() {
        assert_eq!(content_type_for("/tmp/dir/a.png"), "image/png");
        assert_eq!(content_type_for(r"C:\dir\a.zip"), "application/zip");
        assert_eq!(content_type_for("/tmp/.hidden.png"), "image/png");
    }

    #[test]
    fn knownness_follows_the_table() {
        assert!(is_known("a.png"));
        assert!(is_known("a.zip"));
        assert!(!is_known("a.unknown"));
        assert!(!is_known("README"));
    }
}
