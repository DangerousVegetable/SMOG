#[derive(Debug)]
pub enum ClientError {
    AuthenticationError,
    NoConnectionToServer,
    ServerClosedConnection,
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthenticationError => write!(f, "Server-side authentication error"),
            Self::NoConnectionToServer => write!(f, "No connection to server"),
            Self::ServerClosedConnection => write!(f, "Server closed connection"),
        }
    }
}

impl std::error::Error for ClientError {}