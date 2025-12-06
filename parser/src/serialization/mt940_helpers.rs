use chrono::NaiveDate;
use crate::model::{Transaction, Direction, Currency};
use super::common;

/// Преобразует Currency в 3-буквенный код для MT940
pub(crate) fn currency_code(cur: &Currency) -> &'static str {
    match cur {
        Currency::RUB => "RUB",
        Currency::EUR => "EUR",
        Currency::USD => "USD",
        Currency::CNY => "CNY",
        Currency::Other(s) => {
            if s.eq_ignore_ascii_case("rub") {
                "RUB"
            } else if s.eq_ignore_ascii_case("eur") {
                "EUR"
            } else if s.eq_ignore_ascii_case("usd") {
                "USD"
            } else if s.eq_ignore_ascii_case("cny") {
                "CNY"
            } else {
                "XXX"
            }
        }
    }
}

/// Форматируем дату как YYMMDD для MT940
pub(crate) fn format_yymmdd(date: NaiveDate) -> String {
    date.format("%y%m%d").to_string()
}

/// Форматируем одну строку :61: из Transaction
pub(crate) fn format_61_line(tx: &Transaction) -> String {
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
pub(crate) fn format_86_line(tx: &Transaction) -> Option<String> {
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
