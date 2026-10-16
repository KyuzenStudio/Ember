use std::fmt;

#[derive(Debug, Clone)]
pub struct ProjectError(pub(crate) String);

impl ProjectError {
    pub(crate) fn new(message: String) -> Self {
        Self(message)
    }
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ProjectError {}
