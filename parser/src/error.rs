use thiserror::Error;

/// Ошибки при парсинге данных
#[derive(Debug, Error)]
pub enum ParseError {
    /// обёртка csv::Error
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),

    /// обёртка quick_xml::de::DeError
    #[error("Xml deserialization error: {0}")]
    XmlDe(#[from] quick_xml::de::DeError),

    /// обёртка quick_xml::se::SeError
    #[error("Xml serialization error: {0}")]
    XmlSe(#[from] quick_xml::se::SeError),

    /// обёртка chrono::ParseError
    #[error("date parse error: {0}")]
    Date(#[from] chrono::ParseError),

    /// обёртка std::num::ParseIntError
    #[error("number parse error: {0}")]
    Int(#[from] std::num::ParseIntError),

    /// обёртка std::io::Error
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    // логические ошибки

    /// ошибка при парсинге валюты
    #[error("invalid currency: {0}")]
    InvalidCurrency(String),

    /// ошибка при парсинге денежной суммы
    #[error("invalid amount: {0}")]
    InvalidAmount(String),

    /// ошибка при парсинге направления транзакции (дебет/кредит)
    #[error("invalid direction: {0}")]
    InvalidDirection(String),

    /// ошибка отсутствия обязательного поля
    #[error("missing field: {0}")]
    MissingField(&'static str),

    /// ошибка при проверке двойной записи: и дебет, и кредит, или ни одного
    #[error("both debit and credit amount present or both empty")]
    AmountSideConflict,

    /// ошибка парсинга заголовка (csv)
    #[error("invalid header: {0}")]
    Header(String),

    /// очень общая ошибка плохих входных данных
    #[error("bad input: {0}")]
    BadInput(String),

    /// ошибка парсинга тега mt940
    #[error("bad mt940 tag: {0}")]
    Mt940Tag(String),
}

