use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("config error: {0}")]
    Config(String),
    #[error("audit error: {0}")]
    Audit(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
