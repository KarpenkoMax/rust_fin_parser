use crate::ParseError;
use chrono::{Datelike, NaiveDate};
use once_cell::sync::Lazy;
use regex::Regex;

static IBAN_RE: Lazy<Regex> = Lazy::new(|| {
    // (?i) - case-insensitive
    // ^[A-Z]{2} - 2 буквы страны
    // \d{2} - 2 цифры
    // [A-Z0-9]{11,30} - хвост
    Regex::new(r"(?i)^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$").unwrap()
});

/// Разделяет строку с тегом на сам тег и строку после него
pub(crate) fn split_tag_line(line: &str) -> Result<(&str, &str), ParseError> {
    let line = line.trim_start();
    if !line.starts_with(':') {
        return Err(ParseError::Mt940Tag("tag line must start with ':'".into()));
    }

    let rest = &line[1..];
    let tag_end_pos = rest.find(':')
        .ok_or_else(|| ParseError::Mt940Tag(format!("bad tag line (unclosed tag): {line}")))?;

    let (tag_raw, value_with_colon) = rest.split_at(tag_end_pos);
    let tag = tag_raw.trim(); 
    let value = &value_with_colon[1..];  // пропускаем двоеточие
    
    Ok((tag, value))
}

pub(crate) fn parse_mt940_yy_mm_dd(s: &str) -> Result<NaiveDate, ParseError> {
    if s.len() != 6 {
        return Err(ParseError::BadInput(format!(
            "invalid YYMMDD date: '{s}'"
        )));
    }

    let yy: i32 = s[0..2]
        .parse()
        .map_err(|_| ParseError::BadInput(format!("invalid year in YYMMDD: '{s}'")))?;
    let mm: u32 = s[2..4]
        .parse()
        .map_err(|_| ParseError::BadInput(format!("invalid month in YYMMDD: '{s}'")))?;
    let dd: u32 = s[4..6]
        .parse()
        .map_err(|_| ParseError::BadInput(format!("invalid day in YYMMDD: '{s}'")))?;

    // простое допущение: все даты в 2000-х
    let year = 2000 + yy;

    NaiveDate::from_ymd_opt(year, mm, dd).ok_or_else(|| {
        ParseError::BadInput(format!("invalid YYMMDD date components: '{s}'"))
    })
}

pub(crate) fn derive_booking_date(
    value_date: NaiveDate,
    entry_date: Option<&str>,
) -> Result<NaiveDate, ParseError> {
    let Some(ed) = entry_date else {
        // считаем, что дата проводки = value_date
        return Ok(value_date);
    };

    match ed.len() {
        4 => {
            // MMDD
            let mm: u32 = ed[0..2]
                .parse()
                .map_err(|_| ParseError::BadInput(format!("invalid MMDD in entry date: '{ed}'")))?;
            let dd: u32 = ed[2..4]
                .parse()
                .map_err(|_| ParseError::BadInput(format!("invalid MMDD in entry date: '{ed}'")))?;

            let year = value_date.year();

            NaiveDate::from_ymd_opt(year, mm, dd).ok_or_else(|| {
                ParseError::BadInput(format!("invalid MMDD entry date: '{ed}'"))
            })
        }
        2 => {
            // DD, месяц берём из value_date
            let mm = value_date.month();
            let dd: u32 = ed
                .parse()
                .map_err(|_| ParseError::BadInput(format!("invalid DD in entry date: '{ed}'")))?;

            let year = value_date.year();

            NaiveDate::from_ymd_opt(year, mm, dd).ok_or_else(|| {
                ParseError::BadInput(format!("invalid DD entry date: '{ed}'"))
            })
        }
        _ => Err(ParseError::BadInput(format!(
            "entry date must be 2 or 4 digits, got '{ed}'"
        ))),
    }
}

/// Ищет IBAN + имя в наборе строк
pub(crate) fn find_iban_and_name_in_lines(lines: &[String]) -> Option<(String, Option<String>)> {
    // Сначала пытаемся найти строку, где в одной строке есть и IBAN, и часть имени
    for line in lines {
        if let Some((iban, name)) = find_iban_and_name_in_line(line) {
            return Some((iban, name));
        }
    }

    // ищем строку с IBAN
    let mut iban_idx: Option<usize> = None;
    let mut iban_value: Option<String> = None;

    for (idx, line) in lines.iter().enumerate() {
        if let Some(iban) = find_iban_in_line(line) {
            iban_idx = Some(idx);
            iban_value = Some(iban);
            break;
        }
    }

    let iban = iban_value?;

    // ищем имя в следующей непустой строке без IBAN
    let mut name: Option<String> = None;
    if let Some(idx) = iban_idx {
        for line in lines.iter().skip(idx + 1) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if find_iban_in_line(trimmed).is_some() {
                continue;
            }
            name = Some(trimmed.to_string());
            break;
        }
    }

    Some((iban, name))
}

/// В одной строке ищем токен, похожий на IBAN.
/// все, что после считается именем контрагента.
pub(crate) fn find_iban_and_name_in_line(line: &str) -> Option<(String, Option<String>)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    for (idx, &token) in tokens.iter().enumerate() {
        if let Some(iban) = normalize_and_check_iban(token) {
            let name = if idx + 1 < tokens.len() {
                let rest = tokens[idx + 1..].join(" ");
                let rest = rest.trim();
                if rest.is_empty() {
                    None
                } else {
                    Some(rest.to_string())
                }
            } else {
                None
            };

            return Some((iban, name));
        }
    }

    None
}

/// Ищет любой IBAN-подобный токен в строке
pub(crate) fn find_iban_in_line(line: &str) -> Option<String> {
    line.split_whitespace()
        .filter_map(|token| normalize_and_check_iban(token))
        .next()
}

pub(crate) fn normalize_and_check_iban(token: &str) -> Option<String> {
    let cleaned = token
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_uppercase();

    if cleaned.is_empty() {
        return None;
    }

    if IBAN_RE.is_match(&cleaned) {
        Some(cleaned)
    } else {
        None
    }
}
