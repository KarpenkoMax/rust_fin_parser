use super::common;
use crate::error::ParseError;
use crate::model::{Balance, Currency, Direction, Statement};
use chrono::{Datelike, Utc};
use csv::Writer;
use std::io::Write;

const COLS: usize = 23;

pub(super) fn empty_row() -> Vec<String> {
    vec![String::new(); COLS]
}

pub(super) fn format_rus_date(d: chrono::NaiveDate) -> String {
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

pub(super) fn currency_label(cur: &Currency) -> String {
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
pub(super) fn make_party_block(account: &str, name: &str) -> String {
    if account.is_empty() && name.is_empty() {
        return String::new();
    }
    let name_line = if name.is_empty() { "-" } else { name };
    format!("{account}\n-\n{name_line}")
}

/// Хелпер для записи заголовка csv-выписки
pub(super) fn write_header<W: Write>(
    wtr: &mut Writer<W>,
    stmt: &Statement,
) -> Result<(), ParseError> {
    let now = Utc::now();

    let mut row0 = empty_row();
    row0[1] = now.format("%d.%m.%Y").to_string();
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
    let period_from_str = format!("за период с {}", format_rus_date(stmt.period_from));
    let period_until_str = format!("по {}", format_rus_date(stmt.period_until));

    row6[2] = period_from_str;
    row6[14] = "по".to_string();
    row6[15] = period_until_str;
    wtr.write_record(&row6)?;

    let mut row7 = empty_row();
    row7[2] = currency_label(&stmt.currency);

    if let Some(last_date) = stmt.transactions.iter().map(|t| t.booking_date).max() {
        row7[12] = format!(
            "Дата предыдущей операции по счету {}",
            format_rus_date(last_date)
        );
    }

    wtr.write_record(&row7)?;

    // row 8 - пустая строка перед таблицей
    wtr.write_record(empty_row())?;

    Ok(())
}

/// Хелпер для записи футера csv-выписки
pub(super) fn write_footer<W: Write>(
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

        let (debit_minor, credit_minor): (u64, u64) = if opening >= 0 {
            (0, opening as u64)
        } else {
            ((-opening) as u64, 0)
        };

        opening_row[7] = common::format_minor_units(debit_minor, ',');
        opening_row[11] = common::format_minor_units(credit_minor, '.');

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

        let (debit_minor, credit_minor): (u64, u64) = if closing >= 0 {
            (0, closing as u64)
        } else {
            ((-closing) as u64, 0)
        };

        closing_row[7] = common::format_minor_units(debit_minor, ',');
        closing_row[11] = common::format_minor_units(credit_minor, '.');

        closing_row[17] = "(П)".to_string();
        closing_row[19] = format_rus_date(stmt.period_until);
        wtr.write_record(&closing_row)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Currency, Direction, Statement, Transaction};
    use chrono::NaiveDate;
    use csv::ReaderBuilder;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn read_all_records(buf: &[u8]) -> Vec<Vec<String>> {
        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(buf);

        rdr.records()
            .map(|r| r.unwrap().iter().map(|s| s.to_string()).collect())
            .collect()
    }

    #[test]
    fn empty_row_has_correct_length_and_is_empty() {
        let row = empty_row();
        assert_eq!(row.len(), COLS);
        assert!(row.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn format_rus_date_formats_correctly() {
        let date = d(2023, 1, 1);
        assert_eq!(format_rus_date(date), "01 января 2023 г.");

        let date = d(1999, 12, 31);
        assert_eq!(format_rus_date(date), "31 декабря 1999 г.");
    }

    #[test]
    fn currency_label_for_known_currencies() {
        assert_eq!(currency_label(&Currency::RUB), "Российский рубль");
        assert_eq!(currency_label(&Currency::EUR), "Евро");
        assert_eq!(currency_label(&Currency::USD), "Доллар США");
        assert_eq!(currency_label(&Currency::CNY), "Китайский юань");
    }

    #[test]
    fn currency_label_for_other() {
        let cur = Currency::Other("Some Currency".to_string());
        assert_eq!(currency_label(&cur), "Some Currency");
    }

    #[test]
    fn make_party_block_empty_when_no_data() {
        let block = make_party_block("", "");
        assert_eq!(block, "");
    }

    #[test]
    fn make_party_block_with_account_and_name() {
        let block = make_party_block("40702810...", "ООО Ромашка");
        assert_eq!(block, "40702810...\n-\nООО Ромашка");
    }

    #[test]
    fn make_party_block_with_account_only() {
        let block = make_party_block("40702810...", "");
        // имя заменяется на "-"
        assert_eq!(block, "40702810...\n-\n-");
    }

    #[test]
    fn make_party_block_with_name_only() {
        let block = make_party_block("", "ООО Ромашка");
        // первая строка пустая, но блок всё равно из 3 строк
        assert_eq!(block, "\n-\nООО Ромашка");
    }

    fn sample_statement() -> Statement {
        let tx1 = Transaction::new(
            d(2023, 1, 10),
            None,
            100_00,
            Direction::Debit,
            "Payment 1".to_string(),
            None,
            None,
        );

        let tx2 = Transaction::new(
            d(2023, 1, 15),
            None,
            200_00,
            Direction::Credit,
            "Payment 2".to_string(),
            None,
            None,
        );

        Statement::new(
            "40702810XXXXXXXXXXXX".to_string(),
            Some("ООО Ромашка".to_string()),
            Currency::RUB,
            Some(1_000_00),
            Some(900_00),
            vec![tx1, tx2],
            d(2023, 1, 1),
            d(2023, 1, 31),
        )
    }

    #[test]
    fn write_header_writes_expected_rows() {
        let stmt = sample_statement();
        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut wtr = Writer::from_writer(&mut buffer);
            write_header(&mut wtr, &stmt).unwrap();
            wtr.flush().unwrap();
        }

        let records = read_all_records(&buffer);
        // 9 строк: 0..7 + пустая
        assert_eq!(records.len(), 9);

        let row0 = &records[0];
        assert_eq!(row0.len(), COLS);
        // Дата формирования в [1] - просто проверим, что не пустая
        assert!(!row0[1].is_empty());

        let row1 = &records[1];
        assert_eq!(row1[5], "СберБизнес. экспорт выписки");

        let row2 = &records[2];
        assert_eq!(row2[1], "ПАО СБЕРБАНК");

        let row3 = &records[3];
        assert!(row3[1].starts_with("Дата формирования выписки "));

        let row4 = &records[4];
        assert_eq!(row4[1], "ВЫПИСКА ОПЕРАЦИЙ ПО ЛИЦЕВОМУ СЧЕТУ");
        assert_eq!(row4[12], stmt.account_id);

        let row5 = &records[5];
        assert_eq!(row5[12], stmt.account_name.clone().unwrap());

        let row6 = &records[6];
        // "за период с {rus_date_from}"
        let expected_from = format!("за период с {}", format_rus_date(stmt.period_from));
        assert_eq!(row6[2], expected_from);
        assert_eq!(row6[14], "по");
        // "по {rus_date_until}"
        let expected_to = format!("по {}", format_rus_date(stmt.period_until));
        assert_eq!(row6[15], expected_to);

        let row7 = &records[7];
        assert_eq!(row7[2], currency_label(&stmt.currency));

        // Дата предыдущей операции (максимальная дата по транзакциям)
        let last_date = stmt
            .transactions
            .iter()
            .map(|t| t.booking_date)
            .max()
            .unwrap();
        let expected_last = format!(
            "Дата предыдущей операции по счету {}",
            format_rus_date(last_date)
        );
        assert_eq!(row7[12], expected_last);

        // row8 - полностью пустая строка
        let row8 = &records[8];
        assert!(row8.iter().all(|s| s.is_empty()));
    }

    #[test]
    fn write_footer_writes_bs_counts_and_totals_and_balances() {
        let stmt = sample_statement();
        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut wtr = Writer::from_writer(&mut buffer);
            write_footer(&mut wtr, &stmt).unwrap();
            wtr.flush().unwrap();
        }

        let records = read_all_records(&buffer);
        // 5 строк: б/с, Количество операций, Входящий остаток, Итого оборотов, Исходящий остаток
        assert_eq!(records.len(), 5);

        // 0: б/с
        let bs_row = &records[0];
        assert_eq!(bs_row[1], "б/с");
        assert_eq!(bs_row[3], stmt.account_id);
        assert_eq!(bs_row[7], "Дебет");
        assert_eq!(bs_row[11], "Кредит");
        assert_eq!(bs_row[20], "Всего");

        // подсчёт вручную для проверки
        let debit_ops = stmt
            .transactions
            .iter()
            .filter(|t| t.direction == Direction::Debit)
            .count();
        let credit_ops = stmt
            .transactions
            .iter()
            .filter(|t| t.direction == Direction::Credit)
            .count();
        let total_ops = debit_ops + credit_ops;

        let debit_turnover: Balance = stmt
            .transactions
            .iter()
            .filter(|t| t.direction == Direction::Debit)
            .map(|t| t.amount as Balance)
            .sum();

        let credit_turnover: Balance = stmt
            .transactions
            .iter()
            .filter(|t| t.direction == Direction::Credit)
            .map(|t| t.amount as Balance)
            .sum();

        // 1: Количество операций
        let count_row = &records[1];
        assert_eq!(count_row[1], "Количество операций");
        assert_eq!(count_row[6], debit_ops.to_string());
        assert_eq!(count_row[10], credit_ops.to_string());
        assert_eq!(count_row[20], total_ops.to_string());

        // 2: Входящий остаток (если есть)
        let opening_row = &records[2];
        assert_eq!(opening_row[1], "Входящий остаток");
        // в коде дебет для этой строки = 0
        assert_eq!(opening_row[7], common::format_minor_units(0, ','));
        assert_eq!(
            opening_row[11],
            common::format_minor_units(stmt.opening_balance.unwrap() as u64, '.')
        );
        assert_eq!(opening_row[17], "(П)");
        assert_eq!(opening_row[19], format_rus_date(stmt.period_from));

        // 3: Итого оборотов
        let total_row = &records[3];
        assert_eq!(total_row[1], "Итого оборотов");
        assert_eq!(
            total_row[7],
            common::format_minor_units(debit_turnover as u64, '.')
        );
        assert_eq!(
            total_row[11],
            common::format_minor_units(credit_turnover as u64, '.')
        );

        // 4: Исходящий остаток (если есть)
        let closing_row = &records[4];
        assert_eq!(closing_row[1], "Исходящий остаток");
        assert_eq!(closing_row[7], common::format_minor_units(0, ','));
        assert_eq!(
            closing_row[11],
            common::format_minor_units(stmt.closing_balance.unwrap() as u64, '.')
        );
        assert_eq!(closing_row[17], "(П)");
        assert_eq!(closing_row[19], format_rus_date(stmt.period_until));
    }

    #[test]
    fn write_footer_handles_no_balances() {
        // тот же стейтмент, но без opening/closing
        let mut stmt = sample_statement();
        stmt.opening_balance = None;
        stmt.closing_balance = None;

        let mut buffer: Vec<u8> = Vec::new();
        {
            let mut wtr = Writer::from_writer(&mut buffer);
            write_footer(&mut wtr, &stmt).unwrap();
            wtr.flush().unwrap();
        }

        let records = read_all_records(&buffer);
        // только 3 строки: б/с, Количество операций, Итого оборотов
        assert_eq!(records.len(), 3);

        assert_eq!(records[0][1], "б/с");
        assert_eq!(records[1][1], "Количество операций");
        assert_eq!(records[2][1], "Итого оборотов");
    }
}
