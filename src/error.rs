//! Error types for HTML extraction and parsing operations

/// Errors that can occur during HTML extraction
#[derive(Debug, thiserror::Error)]
pub enum ExtractionError {
    /// Failed to parse a CSS selector
    #[error("Failed to parse selector '{selector}': {error}")]
    InvalidSelector { selector: String, error: String },

    /// Required field was not found in the HTML
    #[error("Required field '{field}' not found using selector '{selector}'")]
    MissingField { field: String, selector: String },

    /// Failed to parse extracted text into the target type
    #[error("Failed to parse field '{field}' from text '{text}': {error}")]
    ParseError {
        field: String,
        text: String,
        error: ParseError,
    },

    /// Multiple errors occurred during extraction
    #[error("Multiple errors occurred: {errors:?}")]
    Multiple { errors: Vec<ExtractionError> },
}

/// Errors that can occur when parsing text into Rust types
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// Failed to parse a number
    #[error("Invalid number: {text}")]
    InvalidNumber {
        text: String,
        #[source]
        error: std::num::ParseIntError,
    },

    /// Failed to parse a floating point number
    #[error("Invalid float: {text}")]
    InvalidFloat {
        text: String,
        #[source]
        error: std::num::ParseFloatError,
    },

    /// Failed to parse a boolean value
    #[error("Invalid boolean: {text}")]
    InvalidBool { text: String },

    /// Custom parsing error
    #[error("Custom parse error: {message}")]
    Custom { message: String },
}
