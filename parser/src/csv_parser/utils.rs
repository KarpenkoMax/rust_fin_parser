use crate::model::{Balance, Direction};
use crate::error::ParseError;
use csv::{StringRecord};
use crate::utils::parse_amount;
use chrono::NaiveDate;

pub(super) fn parse_footer_balance(row: &StringRecord) -> Result<Balance, ParseError> {
    let debit_raw  = row.get(7).map(str::trim).unwrap_or("");
    let credit_raw = row.get(11).map(str::trim).unwrap_or("");

    let is_zero = |s: &str| s.is_empty() || s == "0" || s == "0,00" || s == "0.00";

    let has_debit  = !is_zero(debit_raw);
    let has_credit = !is_zero(credit_raw);

    match (has_debit, has_credit) {
        // только дебет - это отрицательный остаток
        (true, false) => {
            let normalized = debit_raw.replace(',', ".");
            let amount = parse_amount(&normalized)? as i128;
            Ok(-amount)
        }
        // только кредит - положительный
        (false, true) => {
            let normalized = credit_raw.replace(',', ".");
            let amount = parse_amount(&normalized)? as i128;
            Ok(amount)
        }
        // обе пустые/нулевые - считаем ноль
        (false, false) => Ok(0),
        (true, true) => Err(ParseError::Header(
            "footer balance row has both debit and credit amounts".into(),
        )),
    }
}

/// Возвращает:
/// - 1-ю непустую строку как номер счёта
/// - 3-ю непустую строку как имя контрагента
pub(super) fn extract_account_and_name(block: &str) -> (Option<String>, Option<String>) {
    let lines: Vec<_> = block
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    let account = lines.get(0).map(|s| (*s).to_string());
    let name    = lines.get(2).map(|s| (*s).to_string());

    (account, name)
}

/// Определяет счёт и имя контрагента:
/// - если наш счёт в дебете - контрагент = (счёт, имя) из кредитового блока
/// - если наш счёт в кредите - контрагент = (счёт, имя) из дебетового блока
/// - иначе - (None, None)
pub(super) fn extract_counterparty_account(
    debit_block: &str,
    credit_block: &str,
    our_account: &str,
) -> (Option<String>, Option<String>) {
    let (debit_acc,  debit_name)  = extract_account_and_name(debit_block);
    let (credit_acc, credit_name) = extract_account_and_name(credit_block);

    // наш счёт в дебете - к нам пришли деньги
    if let Some(acc) = debit_acc.as_deref() {
        if acc == our_account {
            return (credit_acc, credit_name);
        }
    }

    // наш счёт в кредите - от нас ушли деньги
    if let Some(acc) = credit_acc.as_deref() {
        if acc == our_account {
            return (debit_acc, debit_name);
        }
    }

    (None, None)
}

pub(super) fn parse_amount_and_direction(
    debit: Option<&str>,
    credit: Option<&str>,
) -> Result<(u64, Direction), ParseError> {

    fn is_empty(val: Option<&str>) -> bool {
        if let Some(s) = val {
            s.trim().is_empty()
        } else {
            true
        }
    }

    match (debit, credit) {
        // дебет: значение есть и непустое, кредит пустой/отсутствует
        (Some(d), c) if !d.trim().is_empty() && is_empty(c) => {
            let amount = parse_amount(d)?;
            let direction = Direction::Debit;
            Ok((amount, direction))
        },
        // кредит: значение есть и непустое, дебет пустой/отсутствует
        (d, Some(c)) if !c.trim().is_empty() && is_empty(d) => {
            let amount = parse_amount(c)?;
            let direction = Direction::Credit;
            Ok((amount, direction))
        },
        _ => Err(ParseError::AmountSideConflict)
    }
}

pub(super) fn is_footer_row(row: &StringRecord) -> bool {
    row.iter().any(|field| {
        let field = field.trim();
        field == "б/с"
            || field.starts_with("Количество операций")
            || field.starts_with("Входящий остаток")
            || field.starts_with("Исходящий остаток")
            || field.starts_with("Итого оборотов")
    })
}

/// Ищет индекс колонки, содержащей текст
/// 
/// Возвращает первый найденный, если не находит - возвращает ошибку
pub(super) fn find_col(row: &StringRecord, needle: &str) -> Result<usize, ParseError> {
    // сначала ищем точное совпадение
    if let Some(idx) = row
        .iter()
        .position(|field| field.trim() == needle)
    {
        return Ok(idx);
    }

    // точного совпадения нет
    if let Some(idx) = row
        .iter()
        .position(|field| field.contains(needle))
    {
        return Ok(idx);
    }

    Err(ParseError::Header(
        format!("column with header equal to or containing '{needle}' not found"),
    ))
}

pub(super) fn parse_rus_date(raw: &str) -> Result<NaiveDate, ParseError> {
    let s = raw.trim();
    let s = s
        .trim_end_matches(|c: char| c.is_whitespace() || c == '.' || c == 'г')
        .trim();

    let parts: Vec<&str> = s.split_whitespace().collect();

    if parts.len() < 3 {
        return Err(ParseError::Header(format!("invalid date from string {raw}")));
    }

    let day: u32 = parts[0]
        .parse()
        .map_err(|_| ParseError::Header(format!("invalid day part of date str {raw}")))?;

    let year: i32 = parts[2]
        .parse()
        .map_err(|_| ParseError::Header(format!("invalid year part of date str {raw}")))?;

    let month_str = parts[1].to_lowercase();

    let month = match month_str.as_str() {
        "января" => 1,
        "февраля" => 2,
        "марта" => 3,
        "апреля" => 4,
        "мая" => 5,
        "июня" => 6,
        "июля" => 7,
        "августа" => 8,
        "сентября" => 9,
        "октября" => 10,
        "ноября" => 11,
        "декабря" => 12,
        _ => return Err(ParseError::Header(format!("unknown month in date: {raw}"))),
    };

    NaiveDate::from_ymd_opt(year, month, day)
        .ok_or_else(|| ParseError::Header(format!("invalid date: {raw}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv::StringRecord;

    // вспомогательные функции для тестов

    fn row_with_debit_credit(debit: &str, credit: &str) -> StringRecord {
        // нам нужны как минимум столбцы с индексами 7 и 11
        let mut fields = vec![""; 12];
        fields[7] = debit;
        fields[11] = credit;
        StringRecord::from(fields)
    }

    // parse_footer_balance

    #[test]
    fn parse_footer_balance_uses_debit_when_non_zero() {
        let row = row_with_debit_credit("100", "0,00");
        let balance = parse_footer_balance(&row).unwrap();
        // дебетовая сумма в футере трактуется как отрицательный баланс
        assert_eq!(balance, -10000);
    }

    #[test]
    fn parse_footer_balance_uses_credit_when_debit_zero() {
        let row = row_with_debit_credit("0,00", "100");
        let balance = parse_footer_balance(&row).unwrap();
        // кредитовая сумма = положительный баланс
        assert_eq!(balance, 10000);
    }

    #[test]
    fn parse_footer_balance_treats_zero_and_empty_as_zero() {
        let row = row_with_debit_credit("", "0.00");
        let balance = parse_footer_balance(&row).unwrap();
        assert_eq!(balance, 0);
    }

    #[test]
    fn parse_footer_balance_handles_comma_fraction_in_debit() {
        // 100,50 в дебете -> -10050
        let row = row_with_debit_credit("100,50", "0,00");
        let balance = parse_footer_balance(&row).unwrap();
        assert_eq!(balance, -10050);
    }

    #[test]
    fn parse_footer_balance_handles_dot_fraction_in_debit() {
        // 123.45 в дебете -> -12345
        let row = row_with_debit_credit("123.45", "0.00");
        let balance = parse_footer_balance(&row).unwrap();
        assert_eq!(balance, -12345);
    }

    #[test]
    fn parse_footer_balance_handles_comma_fraction_in_credit() {
        // 250,75 в кредите -> +25075
        let row = row_with_debit_credit("0,00", "250,75");
        let balance = parse_footer_balance(&row).unwrap();
        assert_eq!(balance, 25075);
    }

    #[test]
    fn parse_footer_balance_handles_dot_fraction_in_credit() {
        // 999.99 в кредите -> +99999
        let row = row_with_debit_credit("0.00", "999.99");
        let balance = parse_footer_balance(&row).unwrap();
        assert_eq!(balance, 99999);
    }

    #[test]
    fn parse_footer_balance_treats_both_empty_as_zero() {
        // обе колонки пустые/пробелы -> 0
        let row = row_with_debit_credit("   ", "   ");
        let balance = parse_footer_balance(&row).unwrap();
        assert_eq!(balance, 0);
    }

    // extract_account_and_name

    #[test]
    fn extract_account_and_name_picks_1st_and_3rd_nonempty_lines() {
        let block = r#"
            40802810000000000001
            (ignored)
            ООО "Рога и Копыта"
            ещё что-то
        "#;

        let (account, name) = extract_account_and_name(block);
        assert_eq!(account.as_deref(), Some("40802810000000000001"));
        assert_eq!(name.as_deref(), Some("ООО \"Рога и Копыта\""));
    }

    #[test]
    fn extract_account_and_name_returns_none_if_not_enough_lines() {
        let block = "40802810000000000001\n"; // только одна непустая строка
        let (account, name) = extract_account_and_name(block);
        assert_eq!(account.as_deref(), Some("40802810000000000001"));
        assert_eq!(name, None);
    }

    // extract_counterparty_account

    #[test]
    fn extract_counterparty_account_when_our_account_in_debit() {
        let our_account = "OUR_ACC";

        let debit_block = r#"
            OUR_ACC
            something
            Наше юрлицо
        "#;

        let credit_block = r#"
            CP_ACC
            something
            Контрагент
        "#;

        let (cp_acc, cp_name) =
            extract_counterparty_account(debit_block, credit_block, our_account);

        assert_eq!(cp_acc.as_deref(), Some("CP_ACC"));
        assert_eq!(cp_name.as_deref(), Some("Контрагент"));
    }

    #[test]
    fn extract_counterparty_account_when_our_account_in_credit() {
        let our_account = "OUR_ACC";

        let debit_block = r#"
            CP_ACC
            something
            Контрагент
        "#;

        let credit_block = r#"
            OUR_ACC
            something
            Наше юрлицо
        "#;

        let (cp_acc, cp_name) =
            extract_counterparty_account(debit_block, credit_block, our_account);

        assert_eq!(cp_acc.as_deref(), Some("CP_ACC"));
        assert_eq!(cp_name.as_deref(), Some("Контрагент"));
    }

    #[test]
    fn extract_counterparty_account_returns_none_if_our_account_missing() {
        let our_account = "OUR_ACC";

        let debit_block = r#"
            OTHER1
            something
            Кто-то
        "#;

        let credit_block = r#"
            OTHER2
            something
            Кто-то ещё
        "#;

        let (cp_acc, cp_name) =
            extract_counterparty_account(debit_block, credit_block, our_account);

        assert!(cp_acc.is_none());
        assert!(cp_name.is_none());
    }

    // parse_amount_and_direction

    #[test]
    fn parse_amount_and_direction_debit_only() {
        let res = parse_amount_and_direction(Some("100"), None).unwrap();
        assert_eq!(res.0, 10000);
        assert_eq!(res.1, Direction::Debit);
    }

    #[test]
    fn parse_amount_and_direction_credit_only() {
        let res = parse_amount_and_direction(None, Some("200")).unwrap();
        assert_eq!(res.0, 20000);
        assert_eq!(res.1, Direction::Credit);
    }

    #[test]
    fn parse_amount_and_direction_trims_whitespace() {
        let res = parse_amount_and_direction(Some("  300  "), None).unwrap();
        assert_eq!(res.0, 30000);
        assert_eq!(res.1, Direction::Debit);
    }

    #[test]
    fn parse_amount_and_direction_conflict_both_sides_filled() {
        let res = parse_amount_and_direction(Some("100"), Some("200"));
        assert!(matches!(res, Err(ParseError::AmountSideConflict)));
    }

    #[test]
    fn parse_amount_and_direction_conflict_both_empty() {
        let res = parse_amount_and_direction(Some("  "), Some(" "));
        assert!(matches!(res, Err(ParseError::AmountSideConflict)));
    }

    // is_footer_row

    #[test]
    fn is_footer_row_detects_known_footer_markers() {
        let r1 = StringRecord::from(vec!["", "б/с", ""]);
        let r2 = StringRecord::from(vec!["Количество операций всего"]);
        let r3 = StringRecord::from(vec!["Входящий остаток на начало дня"]);
        let r4 = StringRecord::from(vec!["Исходящий остаток на конец дня"]);
        let r5 = StringRecord::from(vec!["Итого оборотов за день"]);

        assert!(is_footer_row(&r1));
        assert!(is_footer_row(&r2));
        assert!(is_footer_row(&r3));
        assert!(is_footer_row(&r4));
        assert!(is_footer_row(&r5));
    }

    #[test]
    fn is_footer_row_returns_false_for_regular_row() {
        let r = StringRecord::from(vec!["Дата", "Описание", "Сумма"]);
        assert!(!is_footer_row(&r));
    }

    // find_col

    #[test]
    fn find_col_exact_match() {
        let row = StringRecord::from(vec!["Дата", "Номер", "Описание операции"]);
        let idx = find_col(&row, "Описание операции").unwrap();
        assert_eq!(idx, 2);
    }

    #[test]
    fn find_col_contains_match_when_no_exact_match() {
        let row = StringRecord::from(vec!["Дата", "Номер", "Описание операции по счёту"]);
        let idx = find_col(&row, "Описание операции").unwrap();
        assert_eq!(idx, 2);
    }

    #[test]
    fn find_col_returns_error_when_not_found() {
        let row = StringRecord::from(vec!["Дата", "Номер", "Описание"]);
        let res = find_col(&row, "Несуществующий заголовок");
        assert!(matches!(res, Err(ParseError::Header(_))));
    }

    // parse_rus_date

     #[test]
    fn parse_rus_date_parses_normal_russian_date_with_g_dot() {
        let d = parse_rus_date("01 января 2023 г.").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
    }

    #[test]
    fn parse_rus_date_parses_without_g_or_dot_and_with_extra_spaces() {
        let d = parse_rus_date("  31 декабря 1999   ").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(1999, 12, 31).unwrap());

        let d = parse_rus_date("31 декабря 1999 г").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(1999, 12, 31).unwrap());

        let d = parse_rus_date("31 декабря 1999.   ").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(1999, 12, 31).unwrap());
    }

    #[test]
    fn parse_rus_date_parses_with_mixed_case_month() {
        let d = parse_rus_date("15 Мая 2020 г.").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2020, 5, 15).unwrap());

        let d = parse_rus_date("15 МАЯ 2020").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2020, 5, 15).unwrap());
    }

    #[test]
    fn parse_rus_date_returns_error_when_not_enough_parts() {
        let err = parse_rus_date("января 2023 г.").unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(
                    msg.contains("invalid date from string"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected Header error, got {other:?}"),
        }
    }

    #[test]
    fn parse_rus_date_returns_error_on_invalid_ymd() {
        let err = parse_rus_date("xx января 2023 г.").unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(
                    msg.contains("invalid day part"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected Header error, got {other:?}"),
        }

        let err = parse_rus_date("01 января xxxx г.").unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(
                    msg.contains("invalid year part"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected Header error, got {other:?}"),
        }

        let err = parse_rus_date("01 янвяря 2023 г.").unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(
                    msg.contains("unknown month"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected Header error, got {other:?}"),
        }

        // 31 июня не существует
        let err = parse_rus_date("31 июня 2023 г.").unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(
                    msg.contains("invalid date:"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected Header error, got {other:?}"),
        }
    }
}

