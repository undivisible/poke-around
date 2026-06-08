use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("peekaboo error: {0}")]
    Peekaboo(#[from] rs_peekaboo::PeekabooError),
    #[error("poke error: {0}")]
    Poke(#[from] rs_poke::Error),
    #[error("url error: {0}")]
    Url(#[from] url::ParseError),
}

impl Error {
    pub fn msg(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }
}
