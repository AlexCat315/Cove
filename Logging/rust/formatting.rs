//! Translation of `Logging/Sources/CoveLogging/Outputs/Formatting.swift`.
use super::configuration::ConsoleFormat;
use super::record::{LogRecord, LogValue};
use std::collections::BTreeMap;

pub struct LogFormatter;

impl LogFormatter {
    pub fn new() -> Self {
        Self
    }

    /// One JSON object followed by a newline, suitable for JSONL.
    pub fn json(&self, record: &LogRecord) -> Result<Vec<u8>, serde_json::Error> {
        let mut data = serde_json::to_vec(record)?;
        data.push(b'\n');
        Ok(data)
    }

    pub fn text(&self, record: &LogRecord) -> String {
        let metadata = if record.metadata.is_empty() {
            String::new()
        } else {
            format!(
                " {}",
                serde_json::to_string(&record.metadata).unwrap_or_default()
            )
        };
        format!(
            "{} {} [{}] {}{}",
            iso8601(record.timestamp),
            record.level.label().to_uppercase(),
            record.label,
            record.message,
            metadata
        )
    }

    /// Console output skips child-process echoes because the CLI already wrote
    /// them to its original stdout/stderr. Android and desktop routing stays in
    /// the native console sink.
    pub fn console(&self, record: &LogRecord, format: ConsoleFormat) {
        if record.location.source == "process.stdout" || record.location.source == "process.stderr" {
            return;
        }
        let text = match format {
            ConsoleFormat::Json => self
                .json(record)
                .map(|bytes| String::from_utf8_lossy(&bytes).trim_end().to_owned())
                .unwrap_or_default(),
            ConsoleFormat::Text => self.text(record),
        };
        crate::system::console_write(record.level.native_index(), &text);
    }
}
impl Default for LogFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata rendered without a map wrapper for the human-readable line.
pub fn metadata_json(metadata: &BTreeMap<String, LogValue>) -> String {
    serde_json::to_string(metadata).unwrap_or_default()
}

/// `ISO8601DateFormatter` with internet date time and fractional seconds.
pub fn iso8601(timestamp: f64) -> String {
    let seconds = timestamp.floor() as i64;
    let nanos = ((timestamp - seconds as f64) * 1_000_000_000.0).round() as u32;
    let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
    let day_seconds = seconds.rem_euclid(86_400);
    let hour = day_seconds / 3600;
    let minute = (day_seconds % 3600) / 60;
    let second = day_seconds % 60;
    format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{:03}Z",
        nanos / 1_000_000
    )
}

/// Howard Hinnant's days-from-civil inverse, valid for the Unix epoch range.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
