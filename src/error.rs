use thiserror::Error;

#[derive(Error, Debug)]
pub enum DoubleBufferError {
    #[error("Backend initialization failed: {0}")]
    InitFailed(String),
}
