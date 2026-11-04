//! Minimal D-Bus wire codec plus the `org.freedesktop.portal.Screenshot`
//! `PickColor` conversation. No D-Bus crate: the session bus is a Unix socket
//! and a handful of messages, so the portal backend stays dependency-free.
//!
//! The module compiles on non-Linux Unix targets in test builds so the codec
//! and the full request/response flow are exercised against a fake bus here,
//! even though the production backend only runs on Linux.

#![allow(dead_code)]

use cove_platform::{ScreenColor, ScreenColorSample};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

const METHOD_CALL: u8 = 1;
const METHOD_RETURN: u8 = 2;
const ERROR: u8 = 3;
const SIGNAL: u8 = 4;

const DBUS_DESTINATION: &str = "org.freedesktop.DBus";
const DBUS_PATH: &str = "/org/freedesktop/DBus";
const PORTAL_DESTINATION: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_SCREENSHOT: &str = "org.freedesktop.portal.Screenshot";
const PORTAL_REQUEST: &str = "org.freedesktop.portal.Request";

// MARK: - Wire writer

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}
impl Writer {
    fn align(&mut self, alignment: usize) {
        while self.bytes.len() % alignment != 0 {
            self.bytes.push(0);
        }
    }
    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }
    fn u32(&mut self, value: u32) {
        self.align(4);
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn f64(&mut self, value: f64) {
        self.align(8);
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
    fn string(&mut self, value: &str) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value.as_bytes());
        self.bytes.push(0);
    }
    fn signature(&mut self, value: &str) {
        self.byte(value.len() as u8);
        self.bytes.extend_from_slice(value.as_bytes());
        self.byte(0);
    }
    fn variant(&mut self, signature: &str, value: impl FnOnce(&mut Self)) {
        self.signature(signature);
        self.align(alignment_of(signature));
        value(self);
    }
    fn field(&mut self, code: u8, signature: &str, value: impl FnOnce(&mut Self)) {
        self.align(8);
        self.byte(code);
        self.variant(signature, value);
        // `(yv)` is 8-aligned, so pad each header field back to that boundary.
        self.align(8);
    }
    fn array(&mut self, element_alignment: usize, build: impl FnOnce(&mut Self)) {
        self.u32(0);
        let length_at = self.bytes.len() - 4;
        self.align(element_alignment);
        let start = self.bytes.len();
        build(self);
        let length = self.bytes.len() - start;
        self.bytes[length_at..length_at + 4].copy_from_slice(&(length as u32).to_le_bytes());
    }
    fn dict_entry(&mut self, key: &str, signature: &str, value: impl FnOnce(&mut Self)) {
        self.align(8);
        self.string(key);
        self.variant(signature, value);
        self.align(8);
    }
}

fn alignment_of(signature: &str) -> usize {
    match signature.as_bytes().first() {
        Some(b'y' | b'g' | b'v') => 1,
        Some(b'n' | b'q') => 2,
        Some(b'b' | b'i' | b'u' | b's' | b'o' | b'a' | b'h') => 4,
        Some(b'x' | b't' | b'd' | b'(' | b'{') => 8,
        _ => 1,
    }
}

fn container_end(signature: &str, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0usize;
    for (index, byte) in signature.bytes().enumerate() {
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(index + 1);
            }
        }
    }
    None
}

/// Splits one complete D-Bus type from the rest of a signature.
fn split_type(signature: &str) -> Option<(&str, &str)> {
    match signature.as_bytes().first()? {
        b'a' => {
            let (inner, _) = split_type(&signature[1..])?;
            let end = 1 + inner.len();
            Some((&signature[..end], &signature[end..]))
        }
        b'(' => {
            let end = container_end(signature, b'(', b')')?;
            Some((&signature[..end], &signature[end..]))
        }
        b'{' => {
            let end = container_end(signature, b'{', b'}')?;
            Some((&signature[..end], &signature[end..]))
        }
        _ => Some((&signature[..1], &signature[1..])),
    }
}

#[derive(Default)]
struct Header<'a> {
    path: Option<&'a str>,
    interface: Option<&'a str>,
    member: Option<&'a str>,
    destination: Option<&'a str>,
    error_name: Option<&'a str>,
    reply_serial: Option<u32>,
    signature: Option<&'a str>,
}

fn build_message(
    kind: u8,
    serial: u32,
    header: &Header<'_>,
    body: impl FnOnce(&mut Writer),
) -> Vec<u8> {
    let mut writer = Writer::default();
    writer.bytes.extend_from_slice(&[b'l', kind, 0, 1]);
    let body_length_at = writer.bytes.len();
    writer.u32(0);
    writer.u32(serial);
    writer.array(8, |writer| {
        // Header fields must be emitted in ascending field-code order.
        if let Some(path) = header.path {
            writer.field(1, "o", |w| w.string(path));
        }
        if let Some(interface) = header.interface {
            writer.field(2, "s", |w| w.string(interface));
        }
        if let Some(member) = header.member {
            writer.field(3, "s", |w| w.string(member));
        }
        if let Some(error) = header.error_name {
            writer.field(4, "s", |w| w.string(error));
        }
        if let Some(reply_serial) = header.reply_serial {
            writer.field(5, "u", |w| w.u32(reply_serial));
        }
        if let Some(destination) = header.destination {
            writer.field(6, "s", |w| w.string(destination));
        }
        if let Some(signature) = header.signature {
            writer.field(8, "g", |w| w.signature(signature));
        }
    });
    let body_start = writer.bytes.len();
    body(&mut writer);
    writer.align(8);
    let body_length = writer.bytes.len() - body_start;
    writer.bytes[body_length_at..body_length_at + 4]
        .copy_from_slice(&(body_length as u32).to_le_bytes());
    writer.bytes
}

// MARK: - Wire reader

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }
    fn align(&mut self, alignment: usize) {
        self.pos = (self.pos + alignment - 1) / alignment * alignment;
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(count).ok_or("D-Bus value overflow")?;
        if end > self.bytes.len() {
            return Err("truncated D-Bus value".into());
        }
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        self.align(2);
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, String> {
        self.align(4);
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, String> {
        self.align(8);
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn f64(&mut self) -> Result<f64, String> {
        self.align(8);
        Ok(f64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn string(&mut self) -> Result<String, String> {
        self.align(4);
        let length = self.u32()? as usize;
        let bytes = self.take(length + 1)?;
        Ok(String::from_utf8_lossy(&bytes[..length]).into_owned())
    }
    fn signature(&mut self) -> Result<String, String> {
        let length = self.byte()? as usize;
        let bytes = self.take(length + 1)?;
        Ok(String::from_utf8_lossy(&bytes[..length]).into_owned())
    }
}

fn skip_value(reader: &mut Reader<'_>, signature: &str) -> Result<(), String> {
    let (first, _) = split_type(signature).ok_or("invalid D-Bus signature")?;
    match first.as_bytes()[0] {
        b'y' => {
            reader.byte()?;
        }
        b'b' | b'i' | b'u' | b'h' => {
            reader.u32()?;
        }
        b'n' | b'q' => {
            reader.u16()?;
        }
        b'x' | b't' | b'd' => {
            reader.u64()?;
        }
        b's' | b'o' => {
            reader.string()?;
        }
        b'g' => {
            reader.signature()?;
        }
        b'v' => {
            let inner = reader.signature()?;
            skip_value(reader, &inner)?;
        }
        b'a' => {
            reader.align(4);
            let length = reader.u32()? as usize;
            let element = &first[1..];
            reader.align(alignment_of(element));
            let end = reader
                .pos
                .checked_add(length)
                .ok_or("D-Bus array overflow")?;
            if end > reader.bytes.len() {
                return Err("D-Bus array overruns the message".into());
            }
            while reader.pos < end {
                // The last struct/dict entry is padded back to its 8-byte
                // alignment; that padding is not another element.
                reader.align(alignment_of(element));
                if reader.pos >= end {
                    break;
                }
                skip_value(reader, element)?;
            }
        }
        b'(' | b'{' => {
            reader.align(8);
            let inner = &first[1..first.len() - 1];
            let mut rest = inner;
            while !rest.is_empty() {
                let (kind, tail) = split_type(rest).ok_or("invalid struct signature")?;
                skip_value(reader, kind)?;
                rest = tail;
            }
        }
        other => return Err(format!("unsupported D-Bus type {other}")),
    }
    Ok(())
}

struct Message {
    kind: u8,
    serial: u32,
    reply_serial: Option<u32>,
    path: Option<String>,
    interface: Option<String>,
    member: Option<String>,
    error: Option<String>,
    body: Vec<u8>,
}

/// Parses one message, returning it and the number of bytes consumed. `None`
/// means "not enough bytes yet"; the caller keeps buffering.
fn parse_message(bytes: &[u8]) -> Result<Option<(Message, usize)>, String> {
    if bytes.len() < 16 {
        return Ok(None);
    }
    if bytes[0] != b'l' {
        return Err("big-endian D-Bus messages are not supported".into());
    }
    let body_length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let header_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let total = 16usize
        .checked_add(header_length)
        .and_then(|value| value.checked_add(body_length))
        .ok_or("D-Bus message length overflow")?;
    if bytes.len() < total {
        return Ok(None);
    }
    let mut message = Message {
        kind: bytes[1],
        serial: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        reply_serial: None,
        path: None,
        interface: None,
        member: None,
        error: None,
        body: Vec::new(),
    };
    let mut reader = Reader::new(&bytes[16..16 + header_length]);
    while reader.pos < header_length {
        reader.align(8);
        if reader.pos >= header_length {
            break;
        }
        let code = reader.byte()?;
        let signature = reader.signature()?;
        match (code, signature.as_str()) {
            (1, "o") => message.path = Some(reader.string()?),
            (2, "s") => message.interface = Some(reader.string()?),
            (3, "s") => message.member = Some(reader.string()?),
            (4, "s") => message.error = Some(reader.string()?),
            (5, "u") => message.reply_serial = Some(reader.u32()?),
            (8, "g") => {
                reader.signature()?;
            }
            _ => skip_value(&mut reader, &signature)?,
        }
    }
    message.body = bytes[16 + header_length..total].to_vec();
    Ok(Some((message, total)))
}

// MARK: - Connection

fn read_line(stream: &mut UnixStream) -> Result<String, String> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream
            .read_exact(&mut byte)
            .map_err(|error| format!("D-Bus authentication read failed: {error}"))?;
        line.push(byte[0]);
        if line.ends_with(b"\r\n") {
            line.truncate(line.len() - 2);
            return Ok(String::from_utf8_lossy(&line).into_owned());
        }
        if line.len() > 4096 {
            return Err("D-Bus authentication line too long".into());
        }
    }
}

struct Connection {
    stream: UnixStream,
    buffer: Vec<u8>,
    serial: u32,
}
impl Connection {
    fn new(mut stream: UnixStream, uid_hex: &str) -> Result<Self, String> {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .and_then(|_| stream.set_write_timeout(Some(Duration::from_secs(5))))
            .map_err(|error| format!("D-Bus socket setup failed: {error}"))?;
        stream
            .write_all(format!("\0AUTH EXTERNAL {uid_hex}\r\n").as_bytes())
            .map_err(|error| format!("D-Bus AUTH failed: {error}"))?;
        let reply = read_line(&mut stream)?;
        if !reply.starts_with("OK ") {
            return Err(format!("D-Bus AUTH rejected: {reply}"));
        }
        stream
            .write_all(b"NEGOTIATE_UNIX_FD\r\n")
            .map_err(|error| format!("D-Bus negotiation failed: {error}"))?;
        let negotiation = read_line(&mut stream)?;
        if !negotiation.starts_with("AGREE_UNIX_FD") && !negotiation.starts_with("ERROR") {
            return Err(format!("D-Bus negotiation rejected: {negotiation}"));
        }
        stream
            .write_all(b"BEGIN\r\n")
            .map_err(|error| format!("D-Bus BEGIN failed: {error}"))?;
        // Bounded reads let the worker thread notice cancellation while the
        // portal waits for the user to pick a pixel.
        stream
            .set_read_timeout(Some(Duration::from_millis(100)))
            .map_err(|error| format!("D-Bus socket setup failed: {error}"))?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
            serial: 0,
        })
    }
    fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1).max(1);
        self.serial
    }
    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stream
            .write_all(bytes)
            .map_err(|error| format!("D-Bus write failed: {error}"))
    }
    fn read_message(&mut self) -> Result<Option<Message>, String> {
        loop {
            if let Some((message, consumed)) = parse_message(&self.buffer)? {
                self.buffer.drain(..consumed);
                return Ok(Some(message));
            }
            let mut chunk = [0u8; 4096];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Err("D-Bus connection closed".into()),
                Ok(count) => self.buffer.extend_from_slice(&chunk[..count]),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(format!("D-Bus read failed: {error}")),
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn open_bus() -> Result<UnixStream, String> {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::SocketAddr;
    let address = std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .map_err(|_| "DBUS_SESSION_BUS_ADDRESS is not set".to_string())?;
    for part in address.split(',') {
        let Some(options) = part
            .strip_prefix("unix:")
            .or_else(|| part.contains("path=").then_some(part))
        else {
            continue;
        };
        for option in options.split(',') {
            if let Some(path) = option.strip_prefix("path=") {
                return UnixStream::connect(path)
                    .map_err(|error| format!("cannot open the session bus: {error}"));
            }
            if let Some(name) = option.strip_prefix("abstract=") {
                let address = SocketAddr::from_abstract_name(name.as_bytes())
                    .map_err(|error| format!("invalid abstract bus address: {error}"))?;
                return UnixStream::connect_addr(&address)
                    .map_err(|error| format!("cannot open the session bus: {error}"));
            }
        }
    }
    Err("the session bus address has no Unix socket".into())
}
#[cfg(not(target_os = "linux"))]
fn open_bus() -> Result<UnixStream, String> {
    Err("the D-Bus portal backend only runs on Linux".into())
}

// MARK: - Portal client

/// `PickColor` is asynchronous on the bus: the method returns a Request object
/// path, then the answer arrives as a `Response` signal on that path.
pub(crate) struct PortalClient {
    connection: Connection,
    signals: Vec<Message>,
}
impl PortalClient {
    pub(crate) fn connect(uid_hex: &str) -> Result<Self, String> {
        Self::from_stream(open_bus()?, uid_hex)
    }
    pub(crate) fn from_stream(stream: UnixStream, uid_hex: &str) -> Result<Self, String> {
        Ok(Self {
            connection: Connection::new(stream, uid_hex)?,
            signals: Vec::new(),
        })
    }

    fn call(
        &mut self,
        path: &str,
        interface: &str,
        member: &str,
        destination: &str,
        signature: &str,
        body: impl FnOnce(&mut Writer),
        cancel: &AtomicBool,
    ) -> Result<Message, String> {
        let serial = self.connection.next_serial();
        let bytes = build_message(
            METHOD_CALL,
            serial,
            &Header {
                path: Some(path),
                interface: Some(interface),
                member: Some(member),
                destination: Some(destination),
                signature: Some(signature),
                ..Header::default()
            },
            body,
        );
        self.connection.send(&bytes)?;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("the colour request was cancelled".into());
            }
            if Instant::now() >= deadline {
                return Err("the portal did not answer in time".into());
            }
            let Some(message) = self.connection.read_message()? else {
                continue;
            };
            if message.kind == SIGNAL {
                self.signals.push(message);
                continue;
            }
            if message.reply_serial != Some(serial) {
                continue;
            }
            return match message.kind {
                METHOD_RETURN => Ok(message),
                ERROR => Err(format!(
                    "portal error: {}",
                    message.error.unwrap_or_else(|| "unknown".into())
                )),
                _ => Err("unexpected D-Bus reply".into()),
            };
        }
    }

    pub(crate) fn add_match(&mut self, cancel: &AtomicBool) -> Result<(), String> {
        let rule = format!("type='signal',interface='{PORTAL_REQUEST}',member='Response'");
        self.call(
            DBUS_PATH,
            DBUS_DESTINATION,
            "AddMatch",
            DBUS_DESTINATION,
            "s",
            |writer| writer.string(&rule),
            cancel,
        )?;
        Ok(())
    }

    /// Starts a system colour-picker session and returns its Request path.
    pub(crate) fn pick_color(
        &mut self,
        token: &str,
        cancel: &AtomicBool,
    ) -> Result<String, String> {
        let reply = self.call(
            PORTAL_PATH,
            PORTAL_SCREENSHOT,
            "PickColor",
            PORTAL_DESTINATION,
            "sa{sv}",
            |writer| {
                writer.string("");
                writer.array(8, |writer| {
                    writer.dict_entry("handle_token", "s", |writer| writer.string(token));
                });
            },
            cancel,
        )?;
        let mut reader = Reader::new(&reply.body);
        reader.string()
    }

    pub(crate) fn wait_response(
        &mut self,
        handle: &str,
        cancel: &AtomicBool,
    ) -> Result<ScreenColorSample, String> {
        let deadline = Instant::now() + Duration::from_secs(5 * 60);
        loop {
            if cancel.load(Ordering::Relaxed) || Instant::now() >= deadline {
                return Ok(ScreenColorSample::Cancelled);
            }
            let message = match self.signals.pop() {
                Some(message) => Some(message),
                None => self.connection.read_message()?,
            };
            let Some(message) = message else { continue };
            if message.kind != SIGNAL
                || message.member.as_deref() != Some("Response")
                || message.path.as_deref() != Some(handle)
            {
                continue;
            }
            return parse_response(&message.body);
        }
    }
}

fn parse_response(body: &[u8]) -> Result<ScreenColorSample, String> {
    let mut reader = Reader::new(body);
    reader.align(8);
    let code = reader.u32()?;
    if code == 1 {
        return Ok(ScreenColorSample::Cancelled);
    }
    if code != 0 {
        return Ok(ScreenColorSample::Failed(format!(
            "the portal rejected the colour request ({code})"
        )));
    }
    // `(u a{sv})`: the array length follows the u32 response directly, then
    // the first dict entry is 8-aligned.
    let length = reader.u32()? as usize;
    reader.align(8);
    let end = reader
        .pos
        .checked_add(length)
        .ok_or("portal results overflow")?;
    let mut color = None;
    while reader.pos < end {
        reader.align(8);
        if reader.pos >= end {
            break;
        }
        let key = reader.string()?;
        let signature = reader.signature()?;
        if key == "color" {
            color = Some(parse_color(&mut reader, &signature)?);
        } else {
            skip_value(&mut reader, &signature)?;
        }
    }
    Ok(color
        .map(ScreenColorSample::Picked)
        .unwrap_or_else(|| ScreenColorSample::Failed("the portal returned no colour".into())))
}

fn parse_color(reader: &mut Reader<'_>, signature: &str) -> Result<ScreenColor, String> {
    let read = |reader: &mut Reader<'_>| -> Result<[f64; 3], String> {
        reader.align(8);
        Ok([reader.f64()?, reader.f64()?, reader.f64()?])
    };
    let components = match signature {
        "(ddd)" | "ddd" => read(reader)?,
        other => {
            skip_value(reader, other)?;
            return Err(format!("unexpected portal colour type {other}"));
        }
    };
    Ok(ScreenColor::new(
        components[0] as f32,
        components[1] as f32,
        components[2] as f32,
        1.0,
    )
    .clamped())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, thread};

    #[test]
    fn a_method_call_round_trips_through_the_parser() {
        let bytes = build_message(
            METHOD_CALL,
            7,
            &Header {
                path: Some(PORTAL_PATH),
                interface: Some(PORTAL_SCREENSHOT),
                member: Some("PickColor"),
                destination: Some(PORTAL_DESTINATION),
                signature: Some("sa{sv}"),
                ..Header::default()
            },
            |writer| {
                writer.string("");
                writer.array(8, |writer| {
                    writer.dict_entry("handle_token", "s", |writer| writer.string("cove7"));
                });
            },
        );
        let (message, consumed) = parse_message(&bytes).unwrap().unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(message.kind, METHOD_CALL);
        assert_eq!(message.serial, 7);
        assert_eq!(message.path.as_deref(), Some(PORTAL_PATH));
        assert_eq!(message.interface.as_deref(), Some(PORTAL_SCREENSHOT));
        assert_eq!(message.member.as_deref(), Some("PickColor"));
        let mut reader = Reader::new(&message.body);
        assert_eq!(reader.string().unwrap(), "");
        reader.align(4);
        let length = reader.u32().unwrap() as usize;
        reader.align(8);
        let end = reader.pos + length;
        reader.align(8);
        assert_eq!(reader.string().unwrap(), "handle_token");
        assert_eq!(reader.signature().unwrap(), "s");
        assert_eq!(reader.string().unwrap(), "cove7");
        assert!(reader.pos <= end);
    }

    #[test]
    fn skip_value_walks_nested_signatures() {
        assert_eq!(split_type("a{sv}i").unwrap(), ("a{sv}", "i"));
        assert_eq!(split_type("(ddd)u").unwrap(), ("(ddd)", "u"));
        assert_eq!(alignment_of("a{sv}"), 4);
        assert_eq!(alignment_of("{sv}"), 8);
        let mut writer = Writer::default();
        writer.array(8, |writer| {
            writer.dict_entry("other", "s", |writer| writer.string("skip me"));
            writer.dict_entry("number", "u", |writer| writer.u32(42));
        });
        let mut reader = Reader::new(&writer.bytes);
        assert!(skip_value(&mut reader, "a{sv}").is_ok());
        assert_eq!(reader.pos, writer.bytes.len());
    }

    #[test]
    fn a_response_signal_decodes_an_srgb_color() {
        let bytes = build_message(
            SIGNAL,
            99,
            &Header {
                path: Some("/org/freedesktop/portal/desktop/request/1/cove1"),
                interface: Some(PORTAL_REQUEST),
                member: Some("Response"),
                signature: Some("ua{sv}"),
                ..Header::default()
            },
            |writer| {
                writer.align(8);
                writer.u32(0);
                writer.array(8, |writer| {
                    writer.dict_entry("color", "(ddd)", |writer| {
                        writer.align(8);
                        writer.f64(0.25);
                        writer.f64(0.5);
                        writer.f64(0.75);
                    });
                });
            },
        );
        let (message, _) = parse_message(&bytes).unwrap().unwrap();
        let sample = parse_response(&message.body).unwrap();
        assert_eq!(sample.color(), Some(ScreenColor::new(0.25, 0.5, 0.75, 1.0)));
    }

    #[test]
    fn the_portal_client_completes_a_pick_against_a_fake_bus() {
        let (client, mut bus) = UnixStream::pair().unwrap();
        let server = thread::spawn(move || {
            // EXTERNAL authentication, then BEGIN.
            let mut greeting = [0u8; 128];
            let count = bus.read(&mut greeting).unwrap();
            assert!(String::from_utf8_lossy(&greeting[..count]).contains("AUTH EXTERNAL"));
            bus.write_all(b"OK c0ffee\r\n").unwrap();
            assert_eq!(read_line(&mut bus).unwrap(), "NEGOTIATE_UNIX_FD");
            bus.write_all(b"AGREE_UNIX_FD\r\n").unwrap();
            assert_eq!(read_line(&mut bus).unwrap(), "BEGIN");

            let read_message = |bus: &mut UnixStream| {
                let mut buffer = Vec::new();
                loop {
                    if let Some((message, consumed)) = parse_message(&buffer).unwrap() {
                        buffer.drain(..consumed);
                        return (message, buffer);
                    }
                    let mut chunk = [0u8; 512];
                    let count = bus.read(&mut chunk).unwrap();
                    assert!(count > 0, "client closed the bus early");
                    buffer.extend_from_slice(&chunk[..count]);
                }
            };
            let (add_match, mut buffer) = read_message(&mut bus);
            assert_eq!(add_match.member.as_deref(), Some("AddMatch"));
            let _ = buffer.drain(..);
            bus.write_all(&build_message(
                METHOD_RETURN,
                100,
                &Header {
                    reply_serial: Some(add_match.serial),
                    ..Header::default()
                },
                |_| {},
            ))
            .unwrap();

            let (pick, _) = read_message(&mut bus);
            assert_eq!(pick.member.as_deref(), Some("PickColor"));
            let handle = "/org/freedesktop/portal/desktop/request/1/cove1";
            bus.write_all(&build_message(
                METHOD_RETURN,
                101,
                &Header {
                    reply_serial: Some(pick.serial),
                    signature: Some("o"),
                    ..Header::default()
                },
                |writer| writer.string(handle),
            ))
            .unwrap();
            bus.write_all(&build_message(
                SIGNAL,
                102,
                &Header {
                    path: Some(handle),
                    interface: Some(PORTAL_REQUEST),
                    member: Some("Response"),
                    signature: Some("ua{sv}"),
                    ..Header::default()
                },
                |writer| {
                    writer.align(8);
                    writer.u32(0);
                    writer.array(8, |writer| {
                        writer.dict_entry("color", "(ddd)", |writer| {
                            writer.align(8);
                            writer.f64(0.1);
                            writer.f64(0.2);
                            writer.f64(0.3);
                        });
                    });
                },
            ))
            .unwrap();
        });

        let mut portal = PortalClient::from_stream(client, "3e8").unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        portal.add_match(&cancel).unwrap();
        let handle = portal.pick_color("cove1", &cancel).unwrap();
        let sample = portal.wait_response(&handle, &cancel).unwrap();
        assert_eq!(sample.color(), Some(ScreenColor::new(0.1, 0.2, 0.3, 1.0)));
        server.join().unwrap();
    }
}
