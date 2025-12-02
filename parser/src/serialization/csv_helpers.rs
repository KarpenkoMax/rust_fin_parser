use chrono::{Datelike, Utc};
use std::io::Write;
use csv::Writer;
use crate::model::{Statement, Direction, Balance, Currency};
use crate::error::ParseError;
use super::common;

const COLS: usize = 23;


pub(crate) fn empty_row() -> Vec<String> {
    vec![String::new(); COLS]
}

pub(crate) fn format_rus_date(d: chrono::NaiveDate) -> String {
    let day = d.day();
    let year = d.year();
    let month = match d.month() {
        1 => "января",
        2 => "февраля",
        3 => "марта",
        4 => "апреля",
        5 => "мая",
        6 => "июня",
        7 => "июля",
        8 => "августа",
        9 => "сентября",
        10 => "октября",
        11 => "ноября",
        12 => "декабря",
        _ => unreachable!(),
    };
    format!("{day:02} {month} {year} г.")
}

pub(crate) fn currency_label(cur: &Currency) -> String {
    match cur {
        Currency::RUB => "Российский рубль".to_string(),
        Currency::EUR => "Евро".to_string(),
        Currency::USD => "Доллар США".to_string(),
        Currency::CNY => "Китайский юань".to_string(),
        Currency::Other(s) => s.clone(),
    }
}

// Блок с реквизитами стороны: делаем 3 непустые строки,
// чтобы extract_account_and_name мог взять 0-ю как счёт, 2-ю как имя.
pub(crate) fn make_party_block(account: &str, name: &str) -> String {
    if account.is_empty() && name.is_empty() {
        return String::new();
    }
    let name_line = if name.is_empty() { "-" } else { name };
    format!("{account}\n-\n{name_line}")
}

/// Хелпер для записи заголовка csv-выписки
pub(crate) fn write_header<W: Write>(
    wtr: &mut Writer<W>,
    stmt: &Statement,
) -> Result<(), ParseError> {

    let now = Utc::now();

    let mut row0 = empty_row();
    row0[0] = now.format("%d.%m.%Y").to_string();
    wtr.write_record(&row0)?;

    let mut row1 = empty_row();
    row1[5] = "СберБизнес. экспорт выписки".to_string();
    wtr.write_record(&row1)?;

    let mut row2 = empty_row();
    row2[1] = "ПАО СБЕРБАНК".to_string();
    wtr.write_record(&row2)?;

    let mut row3 = empty_row();
    row3[1] = format!(
        "Дата формирования выписки {} в {}",
        now.format("%d.%m.%Y"),
        now.format("%H:%M:%S"),
    );
    wtr.write_record(&row3)?;

    let mut row4 = empty_row();
    row4[1] = "ВЫПИСКА ОПЕРАЦИЙ ПО ЛИЦЕВОМУ СЧЕТУ".to_string();
    row4[12] = stmt.account_id.clone();
    wtr.write_record(&row4)?;

    let mut row5 = empty_row();
    row5[12] = stmt.account_name.clone().unwrap_or_default();
    wtr.write_record(&row5)?;

    let mut row6 = empty_row();
    let period_from_str =
        format!("за период с {}", format_rus_date(stmt.period_from));
    let period_until_str =
        format!("по {}", format_rus_date(stmt.period_until));

    row6[2] = period_from_str;
    row6[14] = "по".to_string();
    row6[15] = period_until_str;
    wtr.write_record(&row6)?;

    let mut row7 = empty_row();
    row7[2] = currency_label(&stmt.currency);

    if let Some(last_date) =
        stmt.transactions.iter().map(|t| t.booking_date).max()
    {
        row7[12] = format!(
            "Дата предыдущей операции по счету {}",
            format_rus_date(last_date)
        );
    }

    wtr.write_record(&row7)?;

    // row 8 - пустая строка перед таблицей
    wtr.write_record(&empty_row())?;

    Ok(())
}

/// Хелпер для записи футера csv-выписки
pub(crate) fn write_footer<W: Write>(
    wtr: &mut Writer<W>,
    stmt: &Statement,
) -> Result<(), ParseError> {
    // б/с
    let mut bs_row = empty_row();
    bs_row[1] = "б/с".to_string();
    bs_row[3] = stmt.account_id.clone();
    bs_row[7] = "Дебет".to_string();
    bs_row[11] = "Кредит".to_string();
    bs_row[20] = "Всего".to_string();
    wtr.write_record(&bs_row)?;

    let mut debit_ops: usize = 0;
    let mut credit_ops: usize = 0;
    let mut debit_turnover: Balance = 0;
    let mut credit_turnover: Balance = 0;

    for tx in &stmt.transactions {
        match tx.direction {
            Direction::Debit => {
                debit_ops += 1;
                debit_turnover += tx.amount as Balance;
            }
            Direction::Credit => {
                credit_ops += 1;
                credit_turnover += tx.amount as Balance;
            }
        }
    }

    let total_ops = debit_ops + credit_ops;

    // Количество операций
    let mut count_row = empty_row();
    count_row[1] = "Количество операций".to_string();
    count_row[6] = debit_ops.to_string();
    count_row[10] = credit_ops.to_string();
    count_row[20] = total_ops.to_string();
    wtr.write_record(&count_row)?;

    // Входящий остаток
    if let Some(opening) = stmt.opening_balance {
        let mut opening_row = empty_row();
        opening_row[1] = "Входящий остаток".to_string();

        opening_row[7] = common::format_minor_units(0, ',');
        opening_row[11] = common::format_minor_units(opening as u64, '.');

        opening_row[17] = "(П)".to_string();
        opening_row[19] = format_rus_date(stmt.period_from);
        wtr.write_record(&opening_row)?;
    }

    // Итого оборотов
    let mut total_row = empty_row();
    total_row[1] = "Итого оборотов".to_string();
    total_row[7] = common::format_minor_units(debit_turnover as u64, '.');
    total_row[11] = common::format_minor_units(credit_turnover as u64, '.');
    wtr.write_record(&total_row)?;

    // Исходящий остаток
    if let Some(closing) = stmt.closing_balance {
        let mut closing_row = empty_row();
        closing_row[1] = "Исходящий остаток".to_string();

        closing_row[7] = common::format_minor_units(0, ',');
        closing_row[11] = common::format_minor_units(closing as u64, '.');

        closing_row[17] = "(П)".to_string();
        closing_row[19] = format_rus_date(stmt.period_until);
        wtr.write_record(&closing_row)?;
    }

    Ok(())
}

