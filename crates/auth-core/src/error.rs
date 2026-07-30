use thiserror::Error;

/// Errors from the auth layer.
#[derive(Debug, Error, PartialEq)]
pub enum AuthError {
    #[error("biometric authentication failed: {0}")]
    BiometricFailed(String),

    #[error("biometric authentication cancelled by user")]
    BiometricCancelled,

    #[error("biometric authentication not supported for this level")]
    NotSupported,

    #[error("session key expired")]
    SessionExpired,

    #[error("session key permission denied")]
    PermissionDenied,

    #[error("invalid session key: {0}")]
    InvalidSessionKey(String),

    #[error("internal error: {0}")]
    Internal(String),
}