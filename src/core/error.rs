use std::{fmt, str::Utf8Error};
use roxmltree;

/// The error type for this application.
#[derive(Debug)]
pub enum AppError {
    /// An I/O error occurred.
    Io(std::io::Error),
    /// A UTF-8 parsing error occurred.
    Utf8(std::string::FromUtf8Error),
    /// An SVN command executed successfully but returned a non-zero status,
    /// indicating a logical failure.
    SvnCommandFailed {
        command: String,
        _stdout: String,
        _stderr: String,
    },
    /// The user cancelled the operation from a UI prompt.
    OperationCancelled,
    /// Failed to parse a revision string.
    RevisionParse(String),
    /// A business logic validation error occurred.
    Validation(String),
    /// An XML parsing error occurred.
    XmlParse(roxmltree::Error),
    /// A URL decoding error occurred.
    UrlDecode(Utf8Error),
    /// An ignore error occurred.
    Ignore(ignore::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(err) => write!(f, "I/O Error: {}", err),
            AppError::Utf8(err) => write!(f, "UTF-8 Conversion Error: {}", err),
            AppError::SvnCommandFailed { command, _stdout, _stderr } => {
                write!(f, "SVN command failed: {}\n", command)?;
                // if !stdout.is_empty() {
                //     write!(f, "--- STDOUT ---\n{}\n", stdout)?;
                // }
                // if !stderr.is_empty() {
                //     write!(f, "--- STDERR ---\n{}\n", stderr)?;
                // }
                Ok(())
            }
            AppError::OperationCancelled => write!(f, "Operation cancelled"),
            AppError::RevisionParse(rev) => write!(f, "Failed to parse revision: {}", rev),
            AppError::Validation(msg) => write!(f, "Error: {}", msg),
            AppError::XmlParse(err) => write!(f, "XML Parsing Error: {}", err),
            AppError::UrlDecode(err) => write!(f, "URL/Path Decoding Error: {}", err),
            AppError::Ignore(err) => write!(f, "Ignore Error: {}", err),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err)
    }
}

impl From<std::string::FromUtf8Error> for AppError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        AppError::Utf8(err)
    }
}

impl From<roxmltree::Error> for AppError {
    fn from(err: roxmltree::Error) -> Self {
        AppError::XmlParse(err)
    }
}

impl From<Utf8Error> for AppError {
    fn from(err: Utf8Error) -> Self {
        AppError::UrlDecode(err)
    }
}

impl From<ignore::Error> for AppError {
    fn from(err: ignore::Error) -> Self {
        AppError::Ignore(err)
    }
}

// This makes AppError a "real" error type that can be returned from main.
impl std::error::Error for AppError {}

// We will also define a uniform Result type for our application.
pub type AppResult<T> = Result<T, AppError>;



