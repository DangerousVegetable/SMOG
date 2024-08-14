
#[derive(Debug)]
pub enum ClientError {
    AuthenticationError,
    NoConnectionToServer,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthenticationError => write!(f, "Authentication error from the server-side"),
            Self::NoConnectionToServer => write!(f, "No connection to server"),
        }
    }
}

impl std::error::Error for ClientError {}