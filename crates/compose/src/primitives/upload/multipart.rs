//! `multipart/form-data` encoding for file uploads.
//!
//! The encoder is streaming: each part is written straight into the body buffer
//! and file bytes are copied once. Callers gather bounded file contents before
//! calling in, because `cove-http` sends a `Vec<u8>` body and neither side should
//! pretend a 4 GiB file can be streamed from disk through it.

use std::fmt::Write as _;

/// One non-file field of a multipart body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadField {
    pub name: String,
    pub value: String,
}

impl UploadField {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// One file part: the bytes already read plus the name the server sees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadFilePart {
    /// The form field the file is attached to.
    pub field: String,
    /// The filename the server records, not a path.
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl UploadFilePart {
    pub fn new(
        field: impl Into<String>,
        file_name: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            field: field.into(),
            file_name: file_name.into(),
            content_type: content_type.into(),
            bytes: bytes.into(),
        }
    }
}

/// Why a multipart body could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum MultipartError {
    #[error("a multipart boundary must be 1 to 70 characters and contain no control characters")]
    InvalidBoundary,
    #[error("a field name, filename or content type contains a line break or NUL")]
    InvalidParameter,
    #[error("the body would exceed {limit} bytes")]
    ExceedsLimit { limit: usize },
}

/// The encoded body and the header value that describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultipartBody {
    pub body: Vec<u8>,
    pub content_type: String,
}

/// Encodes `fields` and `files` as `multipart/form-data`.
///
/// `boundary` must be caller-chosen: a random one would make the output
/// untestable, and the transport that owns the request already produces a
/// unique value. The body is rejected once it would pass `limit` bytes, so a
/// large drop fails before the buffer grows.
pub fn encode(
    boundary: &str,
    fields: &[UploadField],
    files: &[UploadFilePart],
    limit: usize,
) -> Result<MultipartBody, MultipartError> {
    if !valid_boundary(boundary) {
        return Err(MultipartError::InvalidBoundary);
    }
    for field in fields {
        if !valid_parameter(&field.name) {
            return Err(MultipartError::InvalidParameter);
        }
    }
    for file in files {
        if !valid_parameter(&file.field)
            || !valid_parameter(&file.file_name)
            || !valid_parameter(&file.content_type)
        {
            return Err(MultipartError::InvalidParameter);
        }
    }

    let mut body = Vec::new();
    for field in fields {
        write_head(&mut body, limit, boundary, |head| {
            let _ = write!(
                head,
                "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                field.name
            );
        })?;
        push_bounded(&mut body, field.value.as_bytes(), limit)?;
        push_bounded(&mut body, b"\r\n", limit)?;
    }
    for file in files {
        write_head(&mut body, limit, boundary, |head| {
            let _ = write!(
                head,
                "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\nContent-Type: {}\r\n\r\n",
                file.field,
                escaped(&file.file_name),
                file.content_type
            );
        })?;
        push_bounded(&mut body, &file.bytes, limit)?;
        push_bounded(&mut body, b"\r\n", limit)?;
    }
    // The closing delimiter has no trailing CRLF: it terminates the body.
    push_bounded(&mut body, format!("--{boundary}--").as_bytes(), limit)?;

    Ok(MultipartBody {
        body,
        content_type: format!("multipart/form-data; boundary={boundary}"),
    })
}

fn write_head(
    body: &mut Vec<u8>,
    limit: usize,
    boundary: &str,
    write_headers: impl FnOnce(&mut String),
) -> Result<(), MultipartError> {
    let mut head = format!("--{boundary}\r\n");
    write_headers(&mut head);
    push_bounded(body, head.as_bytes(), limit)
}

fn push_bounded(body: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Result<(), MultipartError> {
    if body.len().saturating_add(bytes.len()) > limit {
        return Err(MultipartError::ExceedsLimit { limit });
    }
    body.extend_from_slice(bytes);
    Ok(())
}

/// A boundary is 1 to 70 characters from the RFC 2046 subset.
fn valid_boundary(boundary: &str) -> bool {
    (1..=70).contains(&boundary.len())
        && boundary
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"'()+_,-./:=? ".contains(&byte))
}

/// A quoted parameter must not break the header or the header block.
fn valid_parameter(value: &str) -> bool {
    !value
        .bytes()
        .any(|byte| byte == b'\r' || byte == b'\n' || byte == 0)
}

/// Quotes a filename for `Content-Disposition` without letting it escape the
/// quotes or the header line.
fn escaped(name: &str) -> String {
    name.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boundary() -> &'static str {
        "cove-boundary"
    }

    #[test]
    fn fields_and_files_are_framed_with_crlf() {
        let encoded = encode(
            boundary(),
            &[UploadField::new("caption", "hello")],
            &[UploadFilePart::new(
                "file",
                "a.png",
                "image/png",
                vec![1, 2, 3],
            )],
            1024,
        )
        .expect("encodes");
        assert_eq!(
            encoded.content_type,
            "multipart/form-data; boundary=cove-boundary"
        );
        let text = String::from_utf8(encoded.body).expect("utf8");
        assert!(text.starts_with("--cove-boundary\r\n"));
        assert!(text.contains("Content-Disposition: form-data; name=\"caption\"\r\n\r\nhello\r\n"));
        assert!(text.contains(
            "Content-Disposition: form-data; name=\"file\"; filename=\"a.png\"\r\nContent-Type: image/png\r\n\r\n"
        ));
        assert!(text.ends_with("--cove-boundary--"));
    }

    #[test]
    fn a_filename_cannot_escape_its_quotes() {
        let encoded = encode(
            boundary(),
            &[],
            &[UploadFilePart::new("f", "a\"b\r\nc.png", "image/png", vec![]).clone()],
            1024,
        );
        // The line break in the filename is rejected, not passed through.
        assert_eq!(encoded, Err(MultipartError::InvalidParameter));
        let encoded = encode(
            boundary(),
            &[],
            &[UploadFilePart::new("f", "quo\"te.png", "image/png", vec![])],
            1024,
        )
        .expect("encodes");
        let text = String::from_utf8(encoded.body).expect("utf8");
        assert!(text.contains("filename=\"quo\\\"te.png\""));
    }

    #[test]
    fn an_oversized_body_is_rejected_before_growth() {
        let error = encode(
            boundary(),
            &[],
            &[UploadFilePart::new(
                "f",
                "a.bin",
                "application/octet-stream",
                vec![7; 64],
            )],
            32,
        );
        assert_eq!(error, Err(MultipartError::ExceedsLimit { limit: 32 }));
    }

    #[test]
    fn a_boundary_must_be_plain_and_bounded() {
        assert_eq!(
            encode("", &[], &[], 64),
            Err(MultipartError::InvalidBoundary)
        );
        assert_eq!(
            encode("bad\r\nboundary", &[], &[], 64),
            Err(MultipartError::InvalidBoundary)
        );
        assert_eq!(
            encode(&"x".repeat(71), &[], &[], 64),
            Err(MultipartError::InvalidBoundary)
        );
    }

    #[test]
    fn file_bytes_are_copied_verbatim() {
        let bytes = vec![0u8, 255, 13, 10, 45, 45];
        let encoded = encode(
            boundary(),
            &[],
            &[UploadFilePart::new(
                "f",
                "a.bin",
                "application/octet-stream",
                bytes.clone(),
            )],
            1024,
        )
        .expect("encodes");
        assert!(
            encoded
                .body
                .windows(bytes.len())
                .any(|window| window == bytes.as_slice())
        );
    }
}
