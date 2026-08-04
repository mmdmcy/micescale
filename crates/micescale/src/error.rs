use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Operational(String),
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Core(#[from] micescale_core::error::CoreError),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => 2,
            Self::Operational(_) | Self::Core(_) => 1,
        }
    }
}
