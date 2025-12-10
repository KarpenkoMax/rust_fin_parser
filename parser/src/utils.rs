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

        // Всё остальное - как есть:
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
    let int_part = split.next().expect("cleaned is verified to be non-empty so panic! must be impossible to happen");
    let dec_part = split.next().unwrap_or("");
    if split.next().is_some() {
        // больше одной точки - странный формат
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Currency, Direction};
    use crate::error::ParseError;

    // parse_currency

    #[test]
    fn parse_currency_recognizes_rub_variants() {
        assert_eq!(parse_currency("рубль"), Currency::RUB);
        assert_eq!(parse_currency("руб."), Currency::RUB);
        assert_eq!(parse_currency("российский рубль"), Currency::RUB);
        assert_eq!(parse_currency("RUB"), Currency::RUB);
        assert_eq!(parse_currency("rUr"), Currency::RUB);
    }

    #[test]
    fn parse_currency_recognizes_usd_eur_cny() {
        assert_eq!(parse_currency("usd"), Currency::USD);
        assert_eq!(parse_currency("Доллар США"), Currency::USD);
        assert_eq!(parse_currency("EUR"), Currency::EUR);
        assert_eq!(parse_currency("евро"), Currency::EUR);
        assert_eq!(parse_currency("cny"), Currency::CNY);
        assert_eq!(parse_currency("юань"), Currency::CNY);
    }

    #[test]
    fn parse_currency_falls_back_to_other_with_trimmed_original() {
        let cur = parse_currency("  GBP ");
        match cur {
            Currency::Other(s) => assert_eq!(s, "GBP"),
            other => panic!("expected Currency::Other(\"GBP\"), got {:?}", other),
        }
    }

    // parse_amount

    #[test]
    fn parse_amount_plain_integer_and_zero() {
        assert_eq!(parse_amount("0").unwrap(), 0);
        assert_eq!(parse_amount("1").unwrap(), 100);
        assert_eq!(parse_amount("42").unwrap(), 4200);
    }

    #[test]
    fn parse_amount_with_dot_or_comma_fraction() {
        assert_eq!(parse_amount("1.2").unwrap(), 120);
        assert_eq!(parse_amount("1.23").unwrap(), 123);
        assert_eq!(parse_amount("1,2").unwrap(), 120);
        assert_eq!(parse_amount("1,23").unwrap(), 123);
    }

    #[test]
    fn parse_amount_with_spaces_and_thousand_separators() {
        // пробелы как разделитель тысяч
        assert_eq!(parse_amount("1 234,56").unwrap(), 123_456);
        assert_eq!(parse_amount("1 234.56").unwrap(), 123_456);

        // и ',' и '.' одновременно: запятая выкидывается, точка остаётся как разделитель дробной части
        assert_eq!(parse_amount("1,234.56").unwrap(), 123_456);
    }

    #[test]
    fn parse_amount_empty_or_whitespace_is_error() {
        assert!(matches!(parse_amount(""), Err(ParseError::InvalidAmount(_))));
        assert!(matches!(parse_amount("   "), Err(ParseError::InvalidAmount(_))));
    }

    #[test]
    fn parse_amount_negative_is_error() {
        assert!(matches!(parse_amount("-1"), Err(ParseError::InvalidAmount(_))));
        assert!(matches!(parse_amount(" -10,00 "), Err(ParseError::InvalidAmount(_))));
    }

    #[test]
    fn parse_amount_too_many_fraction_digits_is_error() {
        assert!(matches!(parse_amount("1.234"), Err(ParseError::InvalidAmount(_))));
        assert!(matches!(parse_amount("1,234"), Err(ParseError::InvalidAmount(_))));
    }

    #[test]
    fn parse_amount_too_many_dots_is_error() {
        assert!(matches!(parse_amount("1.2.3"), Err(ParseError::InvalidAmount(_))));
    }

    #[test]
    fn parse_amount_non_numeric_int_part_is_int_error() {
        assert!(matches!(parse_amount("abc"), Err(ParseError::Int(_))));
    }

    // parse_signed_balance

    #[test]
    fn parse_signed_balance_credit_is_positive() {
        let v = parse_signed_balance("1.23", Direction::Credit).unwrap();
        assert_eq!(v, 123i128);
    }

    #[test]
    fn parse_signed_balance_debit_is_negative() {
        let v = parse_signed_balance("1.23", Direction::Debit).unwrap();
        assert_eq!(v, -123i128);
    }

    #[test]
    fn parse_signed_balance_propagates_parse_errors() {
        // отрицательное значение внутри должно упасть с InvalidAmount
        let res = parse_signed_balance("-1.00", Direction::Credit);
        assert!(matches!(res, Err(ParseError::InvalidAmount(_))));
    }
}

