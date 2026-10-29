//! Translation of `System/Sources/CoveProcess/PipeRead.swift`.
use super::command::ProcessError;
use std::io::{self, Read};

#[derive(Debug, thiserror::Error)]
pub enum PipeReadError {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub enum PipeRead {}

impl PipeRead {
    /// Waits for one available chunk or EOF, never for a completely filled
    /// buffer. The caller owns the handle and must not close it while a read is
    /// pending.
    pub fn available<R: Read>(
        handle: &mut R,
        maximum_bytes: usize,
    ) -> Result<Vec<u8>, PipeReadError> {
        if maximum_bytes == 0 || maximum_bytes > 33_554_432 {
            return Err(ProcessError::InvalidBudget.into());
        }
        let mut buffer = vec![0_u8; maximum_bytes];
        loop {
            match handle.read(&mut buffer) {
                // A short read is the contract: return whatever is available.
                Ok(count) => {
                    buffer.truncate(count);
                    return Ok(buffer);
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
