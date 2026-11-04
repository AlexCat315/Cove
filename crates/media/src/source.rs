use crate::{MediaError, MediaErrorKind, MediaResult};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);
impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    pub fn check(&self) -> MediaResult<()> {
        if self.is_cancelled() {
            Err(MediaError::new(
                MediaErrorKind::Cancelled,
                "media input cancelled",
            ))
        } else {
            Ok(())
        }
    }
}
/// Implementations must bound blocking reads and cooperate with cancellation.
pub trait MediaInput: Send {
    fn length(&self) -> Option<u64>;
    fn is_seekable(&self) -> bool;
    fn read(&mut self, bytes: &mut [u8], cancellation: &CancellationToken) -> MediaResult<usize>;
    fn seek(&mut self, offset: u64, cancellation: &CancellationToken) -> MediaResult<()>;
}
impl MediaInput for std::io::Cursor<Arc<[u8]>> {
    fn length(&self) -> Option<u64> {
        Some(self.get_ref().len() as u64)
    }
    fn is_seekable(&self) -> bool {
        true
    }
    fn read(&mut self, bytes: &mut [u8], cancellation: &CancellationToken) -> MediaResult<usize> {
        cancellation.check()?;
        Read::read(self, bytes).map_err(|e| MediaError::new(MediaErrorKind::Source, e.to_string()))
    }
    fn seek(&mut self, offset: u64, cancellation: &CancellationToken) -> MediaResult<()> {
        cancellation.check()?;
        if offset > self.get_ref().len() as u64 {
            return Err(MediaError::new(
                MediaErrorKind::InvalidInput,
                "source seek exceeds length",
            ));
        }
        Seek::seek(self, SeekFrom::Start(offset))
            .map(|_| ())
            .map_err(|e| MediaError::new(MediaErrorKind::Source, e.to_string()))
    }
}
pub enum MediaSource {
    File(PathBuf),
    Uri(String),
    Bytes(Arc<[u8]>),
    Input(Box<dyn MediaInput>),
}
impl MediaSource {
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File(path.into())
    }
    pub fn bytes(bytes: impl Into<Arc<[u8]>>) -> Self {
        Self::Bytes(bytes.into())
    }
    pub fn uri(uri: impl Into<String>) -> MediaResult<Self> {
        let uri = uri.into();
        validate_uri(&uri)?;
        Ok(Self::Uri(uri))
    }
}
pub(crate) fn validate_uri(uri: &str) -> MediaResult<()> {
    let valid = uri.split_once(':').is_some_and(|(scheme, _)| {
        !scheme.is_empty()
            && scheme.as_bytes()[0].is_ascii_alphabetic()
            && scheme
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"+.-".contains(&c))
    });
    if !valid || uri.contains('\0') {
        return Err(MediaError::new(
            MediaErrorKind::InvalidInput,
            "media URI must be absolute",
        ));
    }
    Ok(())
}
