use crate::ParseError;
use chrono::{Datelike, NaiveDate};
use once_cell::sync::Lazy;
use regex::Regex;
use lazy_regex::lazy_regex;

/// IBAN в формате:
/// (?i) - case-insensitive
/// ^[A-Z]{2} - 2 буквы страны
/// \d{2} - 2 цифры
/// [A-Z0-9]{11,30} - хвост
static IBAN_RE: Lazy<Regex> = lazy_regex!(r"(?i)^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$");

/// Разделяет строку с тегом на сам тег и строку после него
pub(super) fn split_tag_line(line: &str) -> Result<(&str, &str), ParseError> {
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

pub(super) fn parse_mt940_yy_mm_dd(s: &str) -> Result<NaiveDate, ParseError> {
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

    // простое допущение: 00-79 -> 2000-2079, 80-99 -> 1900-1999
    let year = if yy >= 80 { 1900 + yy } else { 2000 + yy };

    NaiveDate::from_ymd_opt(year, mm, dd).ok_or_else(|| {
        ParseError::BadInput(format!("invalid YYMMDD date components: '{s}'"))
    })
}

pub(super) fn derive_booking_date(
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
pub(super) fn find_iban_and_name_in_lines(lines: &[String]) -> Option<(String, Option<String>)> {
    // Сначала пытаемся найти строку, где в одной строке есть и IBAN, и часть имени.
    // Нас интересуют только случаи, где name.is_some().
    for line in lines {
        if let Some((iban, name)) = find_iban_and_name_in_line(line) {
            if name.is_some() {
                return Some((iban, name));
            }
        }
    }

    // ищем строку с IBAN и пытаемся взять имя из следующей непустой строки.
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
pub(super) fn find_iban_and_name_in_line(line: &str) -> Option<(String, Option<String>)> {
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
pub(super) fn find_iban_in_line(line: &str) -> Option<String> {
    line.split_whitespace()
        .filter_map(|token| normalize_and_check_iban(token))
        .next()
}

pub(super) fn normalize_and_check_iban(token: &str) -> Option<String> {
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

/// Забирает первый символ из rest и сдвигает rest на него.
/// Возвращает Some(ch), если символ есть, иначе None.
pub(super) fn take_char(rest: &mut &str) -> Option<char> {
    let mut iter = rest.chars();
    let ch = iter.next()?;
    *rest = iter.as_str();
    Some(ch)
}

/// Забирает подряд символы, пока pred(ch) == true.
/// Возвращает собранную строку и сдвигает rest.
pub(super) fn take_while(rest: &mut &str, mut pred: impl FnMut(char) -> bool) -> String {
    // Сохраняем исходный срез, чтобы потом нарезать его по байтам
    let s = *rest;

    let mut iter = s.chars().peekable();
    let mut out = String::new();
    let mut consumed = 0usize; // сколько байт мы съели

    while let Some(&ch) = iter.peek() {
        // смотрим на символ, но пока не съедаем его
        if !pred(ch) {
            break;
        }

        out.push(ch);
        consumed += ch.len_utf8(); // увеличиваем байтовый offset
        iter.next(); // теперь уже можно съесть этот символ
    }

    // rest теперь должен указывать на хвост, начинающийся после consumed байт
    *rest = &s[consumed..];

    out
}

// возвращает: (dc_mark, funds_code, amount, оставшийся хвост)
pub(super) fn parse_dc_and_amount<'a>(
    rest: &'a str,
    full_value: &str,
) -> Result<(char, Option<char>, String, &'a str), ParseError> {
    let mut rest = rest;

    // 1) D/C mark
    let dc_mark = take_char(&mut rest).ok_or_else(|| {
        ParseError::BadInput(format!(
            "no debit/credit mark in :61: '{full_value}'"
        ))
    })?;

    // 2) optional funds code (например R в "DR")
    let mut funds_code = None;
    if let Some(next_ch) = rest.chars().next() {
        if next_ch.is_ascii_alphabetic() && next_ch != 'C' && next_ch != 'D' {
            // продвигаем rest на один символ
            let _ = take_char(&mut rest);
            funds_code = Some(next_ch);
        }
    }

    // 3) сумма: подряд идущие цифры/','/'.'
    let amount = take_while(&mut rest, |ch| {
        ch.is_ascii_digit() || ch == ',' || ch == '.'
    });

    if amount.is_empty() {
        return Err(ParseError::BadInput(format!(
            "no amount found in :61: '{full_value}'"
        )));
    }

    Ok((dc_mark, funds_code, amount, rest))
}


#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use crate::ParseError;

    // split_tag_line

    #[test]
    fn split_tag_line_parses_valid_line() {
        let (tag, value) = split_tag_line(":20:ABC").unwrap();
        assert_eq!(tag, "20");
        assert_eq!(value, "ABC");
    }

    #[test]
    fn split_tag_line_trims_leading_spaces_and_tag() {
        let (tag, value) = split_tag_line("   :25: 123456789 ").unwrap();
        assert_eq!(tag, "25");
        // value не триммится внутри функции
        assert_eq!(value, " 123456789 ");
    }

    #[test]
    fn split_tag_line_fails_if_no_leading_colon() {
        let err = split_tag_line("20:ABC").unwrap_err();
        assert!(matches!(err, ParseError::Mt940Tag(_)));
    }

    #[test]
    fn split_tag_line_fails_if_no_second_colon() {
        let err = split_tag_line(":20ABC").unwrap_err();
        assert!(matches!(err, ParseError::Mt940Tag(_)));
    }

    // parse_mt940_yy_mm_dd

    #[test]
    fn parse_mt940_yy_mm_dd_parses_valid_strings() {
        assert_eq!(
            parse_mt940_yy_mm_dd("251101").unwrap(),
            NaiveDate::from_ymd_opt(2025, 11, 1).unwrap()
        );
    }

    #[test]
    fn parse_mt940_yy_mm_dd_fails_when_expected() {
        assert!(matches!(
            parse_mt940_yy_mm_dd("251301"),
            Err(ParseError::BadInput(_))
        ));
        assert!(matches!(
            parse_mt940_yy_mm_dd("251150"),
            Err(ParseError::BadInput(_))
        ));
        assert!(matches!(
            parse_mt940_yy_mm_dd("abdbef"),
            Err(ParseError::BadInput(_))
        ));
        assert!(matches!(
            parse_mt940_yy_mm_dd("абвгдеёжзи"),
            Err(ParseError::BadInput(_))
        ));
        assert!(matches!(
            parse_mt940_yy_mm_dd("1101"),
            Err(ParseError::BadInput(_))
        ));
    }

    // derive_booking_date

    #[test]
    fn derive_booking_date_defaults_to_value_date_when_none() {
        let vd = NaiveDate::from_ymd_opt(2025, 11, 1).unwrap();
        let bd = derive_booking_date(vd, None).unwrap();
        assert_eq!(bd, vd);
    }

    #[test]
    fn derive_booking_date_uses_mmdd_when_4_digits() {
        let vd = NaiveDate::from_ymd_opt(2025, 11, 1).unwrap();
        let bd = derive_booking_date(vd, Some("0205")).unwrap();
        // MMDD -> 02-05 того же года, что и value_date
        assert_eq!(bd, NaiveDate::from_ymd_opt(2025, 2, 5).unwrap());
    }

    #[test]
    fn derive_booking_date_uses_dd_when_2_digits() {
        let vd = NaiveDate::from_ymd_opt(2025, 11, 1).unwrap();
        let bd = derive_booking_date(vd, Some("15")).unwrap();
        assert_eq!(bd, NaiveDate::from_ymd_opt(2025, 11, 15).unwrap());
    }

    #[test]
    fn derive_booking_date_fails_on_invalid_length() {
        assert!(matches!(
            derive_booking_date(
                NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                Some("2")
            ),
            Err(ParseError::BadInput(_))
        ));
        assert!(matches!(
            derive_booking_date(
                NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                Some("010203")
            ),
            Err(ParseError::BadInput(_))
        ));
    }

    #[test]
    fn derive_booking_date_fails_on_invalid_digits() {
        assert!(matches!(
            derive_booking_date(
                NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                Some("zz")
            ),
            Err(ParseError::BadInput(_))
        ));
        assert!(matches!(
            derive_booking_date(
                NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                Some("99aa")
            ),
            Err(ParseError::BadInput(_))
        ));
    }

    // normalize_and_check_iban / find_iban_in_line

    // используем один валидный IBAN без дефисов, только A-Z0-9
    const VALID_IBAN: &str = "DE02123412341234123412";

    #[test]
    fn normalize_and_check_iban_accepts_simple_iban() {
        let iban = normalize_and_check_iban(VALID_IBAN);
        assert_eq!(iban, Some(VALID_IBAN.to_string()));
    }

    #[test]
    fn normalize_and_check_iban_strips_non_alnum_at_edges() {
        let iban = normalize_and_check_iban(&format!("  {VALID_IBAN},"));
        assert_eq!(iban, Some(VALID_IBAN.to_string()));
    }

    #[test]
    fn normalize_and_check_iban_rejects_too_short() {
        let iban = normalize_and_check_iban("DE12999");
        assert!(iban.is_none());
    }

    #[test]
    fn find_iban_in_line_finds_first_iban_like_token() {
        let line = format!("foo {VALID_IBAN} bar");
        let iban = find_iban_in_line(&line);
        assert_eq!(iban, Some(VALID_IBAN.to_string()));
    }

    #[test]
    fn find_iban_in_line_returns_none_if_no_iban() {
        let line = "foo bar baz";
        let iban = find_iban_in_line(line);
        assert!(iban.is_none());
    }

    // find_iban_and_name_in_line

    #[test]
    fn find_iban_and_name_in_line_with_inline_name() {
        let line = format!("{VALID_IBAN} JOHN DOE");
        let (iban, name) = find_iban_and_name_in_line(&line).unwrap();
        assert_eq!(iban, VALID_IBAN);
        assert_eq!(name, Some("JOHN DOE".to_string()));
    }

    #[test]
    fn find_iban_and_name_in_line_without_name() {
        let line = VALID_IBAN;
        let (iban, name) = find_iban_and_name_in_line(line).unwrap();
        assert_eq!(iban, VALID_IBAN);
        assert_eq!(name, None);
    }

    #[test]
    fn find_iban_and_name_in_line_returns_none_if_no_iban() {
        let line = "JOHN DOE ONLY";
        assert!(find_iban_and_name_in_line(line).is_none());
    }

    // find_iban_and_name_in_lines

    #[test]
    fn find_iban_and_name_in_lines_prefers_inline_case() {
        let lines = vec![
            "SOME HEADER".to_string(),
            format!("{VALID_IBAN} JOHN DOE"),
            "SHOULD BE IGNORED".to_string(),
        ];
        let (iban, name) = find_iban_and_name_in_lines(&lines).unwrap();
        assert_eq!(iban, VALID_IBAN);
        assert_eq!(name, Some("JOHN DOE".to_string()));
    }

        #[test]
        fn find_iban_and_name_in_lines_uses_next_line_as_name_if_needed() {
            let lines = vec![
                "SOME HEADER".to_string(),
                format!("IBAN: {VALID_IBAN}"),
                "".to_string(),
                "John Doe Full Name".to_string(),
            ];

            let (iban, name) = find_iban_and_name_in_lines(&lines).unwrap();
            assert_eq!(iban, VALID_IBAN);
            assert_eq!(name, Some("John Doe Full Name".to_string()));
        }

    #[test]
    fn find_iban_and_name_in_lines_returns_none_if_no_iban() {
        let lines = vec![
            "NO IBAN HERE".to_string(),
            "STILL NO IBAN".to_string(),
        ];
        assert!(find_iban_and_name_in_lines(&lines).is_none());
    }

    // take_char

    #[test]
    fn take_char_returns_first_char_and_moves_rest() {
        let mut rest = "ABC";

        assert_eq!(take_char(&mut rest), Some('A'));
        assert_eq!(rest, "BC");

        assert_eq!(take_char(&mut rest), Some('B'));
        assert_eq!(rest, "C");

        assert_eq!(take_char(&mut rest), Some('C'));
        assert_eq!(rest, "");

        // дальше символов нет
        assert_eq!(take_char(&mut rest), None);
        assert_eq!(rest, "");
    }

    #[test]
    fn take_char_handles_utf8_multibyte_char() {
        let mut rest = "Ж9";

        // 'Ж' - многобайтовый символ, проверяем, что rest после него указывает на '9'
        assert_eq!(take_char(&mut rest), Some('Ж'));
        assert_eq!(rest, "9");

        assert_eq!(take_char(&mut rest), Some('9'));
        assert_eq!(rest, "");

        assert_eq!(take_char(&mut rest), None);
    }

    // take_while

    #[test]
    fn take_while_stops_before_first_non_matching_char() {
        let mut rest = "123ABC";

        let prefix = take_while(&mut rest, |ch| ch.is_ascii_digit());

        assert_eq!(prefix, "123");
        assert_eq!(rest, "ABC");
    }

    #[test]
    fn take_while_can_return_empty_and_leave_rest_unchanged() {
        let mut rest = "ABC";

        let prefix = take_while(&mut rest, |ch| ch.is_ascii_digit());

        assert_eq!(prefix, "");
        assert_eq!(rest, "ABC");
    }

    #[test]
    fn take_while_handles_utf8_multibyte_chars() {
        let mut rest = "ЖЖA";

        let prefix = take_while(&mut rest, |ch| !ch.is_ascii());

        assert_eq!(prefix, "ЖЖ");
        assert_eq!(rest, "A");
    }

    // parse_dc_and_amount

    #[test]
    fn parse_dc_and_amount_parses_simple_credit_without_tail() {
        let rest = "C100,00";
        let full = rest;

        let (dc_mark, funds_code, amount, tail) =
            parse_dc_and_amount(rest, full).expect("parse_dc_and_amount failed");

        assert_eq!(dc_mark, 'C');
        assert_eq!(funds_code, None);
        assert_eq!(amount, "100,00");
        assert_eq!(tail, "");
    }

    #[test]
    fn parse_dc_and_amount_parses_debit_with_tail_after_amount() {
        let rest = "D250,00NTRFREF123//BANKREF some extra";
        let full = rest;

        let (dc_mark, funds_code, amount, tail) =
            parse_dc_and_amount(rest, full).expect("parse_dc_and_amount failed");

        assert_eq!(dc_mark, 'D');
        assert_eq!(funds_code, None);
        assert_eq!(amount, "250,00");
        assert!(tail.starts_with("NTRFREF123//BANKREF some extra"));
    }

    #[test]
    fn parse_dc_and_amount_parses_optional_funds_code() {
        let rest = "DR100,00"; // D + funds_code R + amount
        let full = rest;

        let (dc_mark, funds_code, amount, tail) =
            parse_dc_and_amount(rest, full).expect("parse_dc_and_amount failed");

        assert_eq!(dc_mark, 'D');
        assert_eq!(funds_code, Some('R'));
        assert_eq!(amount, "100,00");
        assert_eq!(tail, "");
    }

    #[test]
    fn parse_dc_and_amount_errors_when_amount_missing() {
        // есть только D/C mark, но нет цифр суммы
        let rest = "C";
        let full = rest;

        let result = parse_dc_and_amount(rest, full);

        assert!(result.is_err(), "expected error when amount is missing");
    }
}


