use thiserror::Error;

pub type Result<T> = std::result::Result<T, BlackboardError>;

#[derive(Debug, Error)]
pub enum BlackboardError {
    #[error("entry not found: {0}")]
    EntryNotFound(String),

    #[error("signal not found: {0}")]
    SignalNotFound(String),

    #[error("obligation not found: {0}")]
    ObligationNotFound(String),

    #[error("run not found: {0}")]
    RunNotFound(String),

    #[error("duplicate id: {0}")]
    DuplicateId(String),

    #[error("invalid state: {0}")]
    InvalidState(String),

    #[error("convergence not reached: {0}")]
    ConvergenceFailed(String),

    #[error("persistence error: {0}")]
    Persistence(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl BlackboardError {
    pub fn persistence(message: impl Into<String>) -> Self {
        Self::Persistence(message.into())
    }

    pub fn invalid_state(message: impl Into<String>) -> Self {
        Self::InvalidState(message.into())
    }
}
