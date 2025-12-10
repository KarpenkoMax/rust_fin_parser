use std::{error::Error, io::Error as IoError, fmt};
use chrono::ParseError as ChronoParseError;
use quick_xml::{de::DeError, se::SeError};

/// Ошибки при парсинге данных
#[derive(Debug)]
pub enum ParseError {
    // обёртки

    /// обёртка csv::Error
    Csv(csv::Error),

    /// обёртка quick_xml::de::DeError
    XmlDe(DeError),
    /// обёртка quick_xml::se::SeError
    XmlSe(SeError),
    /// обёртка chrono::ParseError
    Date(chrono::ParseError),
    /// обёртка std::num::ParseIntError
    Int(std::num::ParseIntError),
    /// обёртка std::io::Error
    Io(IoError),

    // логические ошибки

    /// ошибка при парсинге валюты
    InvalidCurrency(String),
    /// ошибка при парсинге денежной суммы
    InvalidAmount(String),
    /// ошибка при парсинге направления транзакции (дебет/кредит)
    InvalidDirection(String),
    /// ошибка отсутствия обязательного поля
    MissingField(&'static str),
    /// ошибка при проверке двойной записи: и дебет, и кредит, или ни одного
    AmountSideConflict, // 
    /// ошибка парсинга заголовка (csv)
    Header(String),
    /// очень общая ошибка плохих входных данных 
    BadInput(String),
    /// ошибка парсинга тега mt940
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
