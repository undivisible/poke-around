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
    #[error("computer-use backend request failed")]
    Peekaboo(#[from] rs_peekaboo::PeekabooError),
    #[error("computer-use protocol request failed")]
    Praefectus(#[from] praefectus::ProtocolError),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peekaboo_errors_should_not_expose_backend_details() {
        let error = Error::from(rs_peekaboo::PeekabooError::System(
            "secret backend path: /private/tmp/capture".to_string(),
        ));

        assert_eq!(error.to_string(), "computer-use backend request failed");
    }
}
