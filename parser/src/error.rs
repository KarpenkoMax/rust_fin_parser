use std::{error::Error, fmt::{self, write}};

#[derive(Debug)]
pub enum ParseError {
    // обёртки
    Csv(csv::Error),
    Date(chrono::ParseError),
    Int(std::num::ParseIntError),

    // логические ошибки
    InvalidCurrency(String),
    InvalidAmount(String),
    InvalidDirection(String),
    MissingField(&'static str),
    AmountSideConflict, // и дебет, и кредит, или ни одного
    Header(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Csv(e) => write!(f, "CSV error: {e}"),
            ParseError::Date(e) => write!(f, "date parse error: {e}"),
            ParseError::Int(e) => write!(f, "number parse error: {e}"),
            ParseError::InvalidCurrency(s) => write!(f, "invalid currency: {s}"),
            ParseError::InvalidAmount(s) => write!(f, "invalid amount: {s}"),
            ParseError::InvalidDirection(s) => write!(f, "invalid direction: {s}"),
            ParseError::MissingField(name) => write!(f, "missing field: {name}"),
            ParseError::AmountSideConflict => {
                write!(f, "both debit and credit amount present or both empty")
            }
            ParseError::Header(msg) => write!(f, "invalid header: {msg}"),
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ParseError::Csv(e) => Some(e),
            ParseError::Date(e) => Some(e),
            ParseError::Int(e) => Some(e),
            _ => None,
        }
    }
}
