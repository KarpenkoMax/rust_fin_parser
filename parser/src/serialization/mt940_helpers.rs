use chrono::NaiveDate;
use crate::model::{Transaction, Direction, Currency};
use super::common;

/// Преобразует Currency в 3-буквенный код для MT940
pub(super) fn currency_code(cur: &Currency) -> &'static str {
    match cur {
        Currency::RUB => "RUB",
        Currency::EUR => "EUR",
        Currency::USD => "USD",
        Currency::CNY => "CNY",
        Currency::Other(c) => {
            println!("found unknown currency {c} while converting to mt940. using placeholder 'XXX'");
            "XXX"
        }
    }
}

/// Форматируем дату как YYMMDD для MT940
pub(super) fn format_yymmdd(date: NaiveDate) -> String {
    date.format("%y%m%d").to_string()
}

/// Форматируем одну строку :61: из Transaction
pub(super) fn format_61_line(tx: &Transaction) -> String {
    // value_date: берём tx.value_date, если есть, иначе booking_date
    let value_date = tx.value_date.unwrap_or(tx.booking_date);
    let value_part = format_yymmdd(value_date);

    // entry_date: MMDD из booking_date
    let entry_part = tx.booking_date.format("%m%d").to_string();

    // D / C
    let dc_mark = match tx.direction {
        Direction::Debit => 'D',
        Direction::Credit => 'C',
    };

    // Сумма в формате "1234,56" (с разделителем ',')
    let amount_str = common::format_minor_units(tx.amount, ',');

    format!("{value_part}{entry_part}{dc_mark}{amount_str}")
}

/// Формирует строку :86: на основе контрагента и описания.
/// Очень упрощённо: "[IBAN/счёт] [имя] // описание"
pub(super) fn format_86_line(tx: &Transaction) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    if let Some(cp_acc) = &tx.counterparty {
        let cp_acc = cp_acc.trim();
        if !cp_acc.is_empty() {
            parts.push(cp_acc.to_string());
        }
    }

    if let Some(cp_name) = &tx.counterparty_name {
        let cp_name = cp_name.trim();
        if !cp_name.is_empty() {
            parts.push(cp_name.to_string());
        }
    }

    let mut base = parts.join(" ");

    if !tx.description.trim().is_empty() {
        if !base.is_empty() {
            base.push_str(" // ");
        }
        base.push_str(tx.description.trim());
    }

    let base = base.trim().to_string();
    if base.is_empty() {
        None
    } else {
        Some(base)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Transaction, Direction, Currency};
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn currency_code_known_currencies() {
        assert_eq!(currency_code(&Currency::RUB), "RUB");
        assert_eq!(currency_code(&Currency::EUR), "EUR");
        assert_eq!(currency_code(&Currency::USD), "USD");
        assert_eq!(currency_code(&Currency::CNY), "CNY");
    }

    #[test]
    fn currency_code_other_currency_uses_placeholder() {
        let cur = Currency::Other("ABC".to_string());
        assert_eq!(currency_code(&cur), "XXX");
    }

    #[test]
    fn format_yymmdd_formats_correctly() {
        assert_eq!(format_yymmdd(d(2023, 4, 19)), "230419");
        assert_eq!(format_yymmdd(d(1999, 12, 31)), "991231");
        assert_eq!(format_yymmdd(d(2000, 1, 1)), "000101");
    }

    fn tx(
        booking_date: NaiveDate,
        value_date: Option<NaiveDate>,
        amount: u64,
        direction: Direction,
        description: &str,
        counterparty: Option<&str>,
        counterparty_name: Option<&str>,
    ) -> Transaction {
        Transaction::new(
            booking_date,
            value_date,
            amount,
            direction,
            description.to_string(),
            counterparty.map(|s| s.to_string()),
            counterparty_name.map(|s| s.to_string()),
        )
    }

    #[test]
    fn format_61_line_uses_booking_date_when_value_date_absent() {
        let booking = d(2023, 4, 19);
        let t = tx(
            booking,
            None,
            12_345,
            Direction::Credit,
            "Test",
            None,
            None,
        );

        let line = format_61_line(&t);
        // value_date = booking_date => 230419, entry_date = 0419, C, amount 123,45
        assert_eq!(line, "2304190419C123,45");
    }

    #[test]
    fn format_61_line_uses_separate_value_and_entry_dates() {
        let booking = d(2023, 4, 19);
        let value = d(2023, 4, 18);
        let t = tx(
            booking,
            Some(value),
            500,
            Direction::Debit,
            "Test",
            None,
            None,
        );

        let line = format_61_line(&t);
        // value_date = 230418, entry_date = 0419, D, amount 5,00
        assert_eq!(line, "2304180419D5,00");
    }

    #[test]
    fn format_61_line_credit_and_debit_marks() {
        let t_credit = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Credit,
            "",
            None,
            None,
        );
        let t_debit = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Debit,
            "",
            None,
            None,
        );

        let line_c = format_61_line(&t_credit);
        let line_d = format_61_line(&t_debit);

        assert!(line_c.contains('C'));
        assert!(line_d.contains('D'));
        assert_ne!(line_c, line_d);
    }

    #[test]
    fn format_86_line_returns_none_when_all_empty() {
        let t = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Credit,
            "",
            None,
            None,
        );

        assert_eq!(format_86_line(&t), None);
    }

    #[test]
    fn format_86_line_with_only_description() {
        let t = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Credit,
            "Just description",
            None,
            None,
        );

        assert_eq!(
            format_86_line(&t),
            Some("Just description".to_string())
        );
    }

    #[test]
    fn format_86_line_with_account_and_name_no_description() {
        let t = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Credit,
            "",
            Some("DE89370400440532013000"),
            Some("John Doe"),
        );

        assert_eq!(
            format_86_line(&t),
            Some("DE89370400440532013000 John Doe".to_string())
        );
    }

    #[test]
    fn format_86_line_with_all_parts() {
        let t = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Credit,
            "Invoice 123",
            Some("DE89370400440532013000"),
            Some("John Doe"),
        );

        assert_eq!(
            format_86_line(&t),
            Some("DE89370400440532013000 John Doe // Invoice 123".to_string())
        );
    }

    #[test]
    fn format_86_line_trims_parts_and_ignores_empty() {
        let t = tx(
            d(2023, 1, 1),
            None,
            100,
            Direction::Credit,
            "  Desc  ",
            Some("   "),
            Some("  Name  "),
        );

        // пустой account (после trim) должен игнорироваться
        assert_eq!(
            format_86_line(&t),
            Some("Name // Desc".to_string())
        );
    }
}

