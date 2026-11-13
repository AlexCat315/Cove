//! Debug Adapter Protocol `Content-Length` framing.
use crate::error::{DebuggerError, Result};
use serde_json::Value;
use std::io::{BufRead, Write};

/// Largest DAP message this client reads or writes.
pub(crate) const MAX_MESSAGE: usize = 8 * 1024 * 1024;
const MAX_HEADER: usize = 8 * 1024;
const MAX_HEADERS: usize = 32;

fn protocol(message: impl Into<String>) -> DebuggerError {
    DebuggerError::new("protocol_error", message)
}

/// Reads one framed JSON object.
///
/// `Ok(None)` is a clean end of stream. A partial header or body, an oversized
/// frame or invalid JSON is an error; nothing is guessed.
pub(crate) fn read_message<R: BufRead>(reader: &mut R) -> Result<Option<Value>> {
    let mut length = None;
    let mut header_bytes = 0usize;
    let mut headers = 0usize;
    loop {
        let mut line = Vec::new();
        let read = reader.read_until(b'\n', &mut line)?;
        if read == 0 {
            if length.is_some() || header_bytes > 0 {
                return Err(protocol("The adapter closed during a message header."));
            }
            return Ok(None);
        }
        header_bytes += read;
        if header_bytes > MAX_HEADER {
            return Err(protocol("The message header exceeds the size limit."));
        }
        headers += 1;
        if headers > MAX_HEADERS {
            return Err(protocol("The message has too many headers."));
        }
        let line = line.strip_suffix(b"\n").unwrap_or(&line);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            break;
        }
        let text =
            std::str::from_utf8(line).map_err(|_| protocol("The message header is not UTF-8."))?;
        let (name, value) = text
            .split_once(':')
            .ok_or_else(|| protocol("The message header has no colon."))?;
        if name.eq_ignore_ascii_case("Content-Length") {
            let value = value.trim();
            let parsed = value
                .parse::<usize>()
                .map_err(|_| protocol("Content-Length is not a number."))?;
            if parsed > MAX_MESSAGE {
                return Err(protocol(format!(
                    "Content-Length {parsed} exceeds the {MAX_MESSAGE}-byte limit."
                )));
            }
            if length.replace(parsed).is_some() {
                return Err(protocol("The message repeats Content-Length."));
            }
        }
    }
    let length = length.ok_or_else(|| protocol("The message has no Content-Length."))?;
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            protocol("The adapter closed inside a message body.")
        } else {
            error.into()
        }
    })?;
    let message: Value = serde_json::from_slice(&body)
        .map_err(|error| protocol(format!("The message body is not JSON: {error}")))?;
    if !message.is_object() {
        return Err(protocol("The message body is not a JSON object."));
    }
    Ok(Some(message))
}

/// Writes one framed JSON object and flushes it.
pub(crate) fn write_message<W: Write>(writer: &mut W, message: &Value) -> Result<()> {
    let body = serde_json::to_vec(message)
        .map_err(|error| protocol(format!("Cannot encode a DAP message: {error}")))?;
    if body.len() > MAX_MESSAGE {
        return Err(protocol(format!(
            "The DAP message is {} bytes, above the {MAX_MESSAGE}-byte limit.",
            body.len()
        )));
    }
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn reads_a_message_with_extra_headers_and_lf_endings() {
        let mut input = Cursor::new(
            b"Content-Type: application/vscode-jsonrpc\nContent-Length: 12\n\n{\"type\":\"x\"}"
                .to_vec(),
        );
        let message = read_message(&mut input).unwrap().unwrap();
        assert_eq!(message["type"], "x");
    }

    #[test]
    fn a_zero_length_body_is_a_protocol_error() {
        let mut input = Cursor::new(b"Content-Length: 0\r\n\r\n".to_vec());
        assert_eq!(
            read_message(&mut input).unwrap_err().code(),
            "protocol_error"
        );
    }

    #[test]
    fn a_missing_content_length_is_a_protocol_error() {
        let mut input = Cursor::new(b"Content-Type: x\r\n\r\n{}".to_vec());
        assert_eq!(
            read_message(&mut input).unwrap_err().code(),
            "protocol_error"
        );
    }

    #[test]
    fn an_oversized_frame_is_refused_before_reading_the_body() {
        let mut input =
            Cursor::new(format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE + 1).into_bytes());
        let error = read_message(&mut input).unwrap_err();
        assert_eq!(error.code(), "protocol_error");
        assert!(error.message().contains("exceeds"));
    }

    #[test]
    fn eof_before_any_header_is_clean() {
        let mut input = Cursor::new(Vec::new());
        assert!(read_message(&mut input).unwrap().is_none());
    }

    #[test]
    fn round_trips_a_message() {
        let mut output = Vec::new();
        write_message(&mut output, &serde_json::json!({"seq": 1})).unwrap();
        assert_eq!(
            String::from_utf8(output.clone()).unwrap(),
            "Content-Length: 9\r\n\r\n{\"seq\":1}"
        );
        let message = read_message(&mut Cursor::new(output)).unwrap().unwrap();
        assert_eq!(message["seq"], 1);
    }
}
