use thiserror::Error;

#[derive(Error, Debug)]
pub enum VideoBufferError {
    #[error("Backend initialization failed: {0}")]
    InitFailed(String),
    #[error("Backend not initialized")]
    NotInitialized,
    #[error("Present failed: {0}")]
    PresentFailed(String),
}
