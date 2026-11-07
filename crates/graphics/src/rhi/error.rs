//! Translation of NativeRHI/RHIError.swift.
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum RhiError {
    #[error("RHI: unsupported backend — {0}")]
    UnsupportedBackend(String),
    #[error("RHI: unsupported feature — {0}")]
    UnsupportedFeature(String),
    #[error("RHI: invalid argument — {0}")]
    InvalidArgument(String),
    #[error("RHI: layout mismatch — {0}")]
    LayoutMismatch(String),
    #[error("RHI: out of memory")]
    OutOfMemory,
    #[error("RHI: swapchain acquire failed — {0}")]
    SwapchainAcquireFailed(String),
    #[error("RHI: swapchain creation failed — {0}")]
    MakeSwapchainFailed(String),
    #[error("RHI: submit failed — {0}")]
    SubmitFailed(String),
    #[error("RHI: present failed — {0}")]
    PresentFailed(String),
    #[error("RHI: device lost")]
    DeviceLost,
    #[error("RHI: frame not active")]
    FrameNotActive,
    /// A background preparation was cancelled by its caller.
    #[error("RHI: cancelled")]
    Cancelled,
}

impl RhiError {
    pub fn unsupported_backend(detail: impl Into<String>) -> Self {
        Self::UnsupportedBackend(detail.into())
    }
    pub fn unsupported_feature(detail: impl Into<String>) -> Self {
        Self::UnsupportedFeature(detail.into())
    }
    pub fn invalid_argument(detail: impl Into<String>) -> Self {
        Self::InvalidArgument(detail.into())
    }
    pub fn layout_mismatch(detail: impl Into<String>) -> Self {
        Self::LayoutMismatch(detail.into())
    }
    pub fn submit_failed(detail: impl Into<String>) -> Self {
        Self::SubmitFailed(detail.into())
    }
    pub fn present_failed(detail: impl Into<String>) -> Self {
        Self::PresentFailed(detail.into())
    }
    pub fn make_swapchain_failed(detail: impl Into<String>) -> Self {
        Self::MakeSwapchainFailed(detail.into())
    }
}

pub type RhiResult<T> = Result<T, RhiError>;

/// The `rhiRequire` precondition used across the RHI: a failed check is an
/// `invalidArgument` error rather than a crash.
pub fn rhi_require(condition: bool, message: &str) -> RhiResult<()> {
    if condition {
        Ok(())
    } else {
        Err(RhiError::InvalidArgument(message.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_match_the_swift_descriptions() {
        assert_eq!(RhiError::OutOfMemory.to_string(), "RHI: out of memory");
        assert_eq!(
            RhiError::InvalidArgument("bad".into()).to_string(),
            "RHI: invalid argument — bad"
        );
    }

    #[test]
    fn require_reports_invalid_argument() {
        assert!(rhi_require(true, "ok").is_ok());
        assert_eq!(
            rhi_require(false, "nope"),
            Err(RhiError::InvalidArgument("nope".into()))
        );
    }
}
