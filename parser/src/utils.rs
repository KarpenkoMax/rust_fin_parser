use crate::model::{Currency, Direction, Balance};
use crate::error::ParseError;

pub(crate) fn parse_currency(raw: &str) -> Currency {
    let s = raw.trim();
    let lower = s.to_lowercase();

    match lower.as_str() {
        "российский рубль" | "рубль" | "руб." | "rub" | "rur" => Currency::RUB,
        "американский доллар" | "доллар сша" | "usd" => Currency::USD,
        "евро" | "eur" => Currency::EUR,
        "китайский юань" | "юань" | "cny" => Currency::CNY,

        // Всё остальное — как есть:
        _ => Currency::Other(s.to_string()),
    }
}

pub(crate) fn parse_amount(raw: &str) -> Result<u64, ParseError> {
    let mut cleaned = raw.trim().replace(' ', "");

    if raw.contains(',') {
        if raw.contains('.'){
            cleaned = cleaned.replace(',', "");
        } else {
            cleaned = cleaned.replace(',', ".");
        }
    }

    if cleaned.is_empty() {
        return Err(ParseError::InvalidAmount("empty amount".into()));
    }
    if cleaned.starts_with('-'){
        return Err(ParseError::InvalidAmount(format!("negative amount: {cleaned}")));
    }

    let mut split = cleaned.split('.');
    // cleaned точно не пусто, так что ошибки здесь быть не может
    let int_part = split.next().unwrap();
    let dec_part = split.next().unwrap_or("");
    if split.next().is_some() {
        // больше одной точки — странный формат
        return Err(ParseError::InvalidAmount(format!("too many dots in amount: {cleaned}")));
    }

    let int_part: u64 = int_part.parse()?;

    let dec_part: u64 = match dec_part.len() {
        0 => 0,
        1 => {
            let d = dec_part
                .chars()
                .next()
                .and_then(|c| c.to_digit(10))
                .ok_or_else(|| ParseError::InvalidAmount(format!("invalid fractional part: {cleaned}")))?;
            d as u64 * 10
        },
        2 => {
            dec_part
                .parse()?
        },
        _ => {
            return Err(ParseError::InvalidAmount(format!("too many fractional digits in amount: {cleaned}")));
        }
    };

    Ok(int_part * 100 + dec_part)
}

pub(crate) fn parse_signed_balance(
    raw: &str,
    direction: Direction,
) -> Result<Balance, ParseError> {
    let minor = parse_amount(raw)? as i128;

    let signed = match direction {
        Direction::Credit => minor,
        Direction::Debit  => -minor,
    };

    Ok(signed)
}
