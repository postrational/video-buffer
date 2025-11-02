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

#[cfg(feature = "wasm-canvas-backend")]
impl From<VideoBufferError> for wasm_bindgen::JsValue {
    fn from(err: VideoBufferError) -> Self {
        wasm_bindgen::JsValue::from_str(&err.to_string())
    }
}
