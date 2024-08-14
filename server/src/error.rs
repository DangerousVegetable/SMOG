#[derive(Debug)]
pub enum ServerError {
    AuthenticationError,
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthenticationError => write!(f, "Client-side authentication error"),
        }
    }
}

impl std::error::Error for ServerError {}