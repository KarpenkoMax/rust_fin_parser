use std::{error::Error, io::Error as IoError, fmt};
use chrono::ParseError as ChronoParseError;
use quick_xml::{de::DeError, se::SeError};

#[derive(Debug)]
pub enum ParseError {
    // обёртки
    Csv(csv::Error),
    XmlDe(DeError),
    XmlSe(SeError),
    Date(chrono::ParseError),
    Int(std::num::ParseIntError),
    Io(IoError),

    // логические ошибки
    InvalidCurrency(String),
    InvalidAmount(String),
    InvalidDirection(String),
    MissingField(&'static str),
    AmountSideConflict, // и дебет, и кредит, или ни одного
    Header(String),
    BadInput(String),
    Mt940Tag(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Csv(e) => write!(f, "CSV error: {e}"),
            ParseError::XmlDe(e) => write!(f, "Xml deserialization error: {e}"),
            ParseError::XmlSe(e) => write!(f, "Xml serialization error: {e}"),
            ParseError::Date(e) => write!(f, "date parse error: {e}"),
            ParseError::Int(e) => write!(f, "number parse error: {e}"),
            ParseError::Io(e) => write!(f, "io error: {e}"),
            ParseError::InvalidCurrency(s) => write!(f, "invalid currency: {s}"),
            ParseError::InvalidAmount(s) => write!(f, "invalid amount: {s}"),
            ParseError::InvalidDirection(s) => write!(f, "invalid direction: {s}"),
            ParseError::MissingField(name) => write!(f, "missing field: {name}"),
            ParseError::AmountSideConflict => {
                write!(f, "both debit and credit amount present or both empty")
            }
            ParseError::Header(msg) => write!(f, "invalid header: {msg}"),
            ParseError::BadInput(msg) => write!(f, "bad input: {msg}"),
            ParseError::Mt940Tag(msg) => write!(f, "bad mt940 tag: {msg}"),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::Csv(e) => Some(e),
            ParseError::XmlDe(e) => Some(e),
            ParseError::XmlSe(e) => Some(e),
            ParseError::Date(e) => Some(e),
            ParseError::Int(e) => Some(e),
            ParseError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ChronoParseError> for ParseError {
    fn from(e: ChronoParseError) -> Self {
        ParseError::Date(e)
    }
}

impl From<std::num::ParseIntError> for ParseError {
    fn from(e: std::num::ParseIntError) -> Self {
        ParseError::Int(e)
    }
}

impl From<IoError> for ParseError {
    fn from(e: IoError) -> Self {
        ParseError::Io(e)
    }
}
