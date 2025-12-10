mod utils;

use crate::error::ParseError;
use crate::model::{Balance, Statement, Transaction};
use crate::utils::parse_currency;
use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use std::io::Read;
use utils::*;

/// Структура с данными из заголовка CSV-выписки
#[derive(Debug, Default)]
pub(crate) struct CsvHeader {
    creation_date: String,
    system: String,
    bank: String,
    client_account: String,
    client_name: String,
    period_from: String,
    period_until: String,
    currency: String,
    last_transaction_date: String,
}

impl CsvHeader {
    /// Формирует поля выписки из данных заголовка csv-файла
    ///
    /// Ожидает строго определённое расположение полей в заголовке
    fn from_string_records(rows: &[StringRecord]) -> Result<Self, ParseError> {
        if rows.len() < 8 {
            return Err(ParseError::Header("invalid header: not enough rows".into()));
        }

        // хелпер
        let get = |row_idx: usize, col_idx: usize| -> String {
            rows[row_idx].get(col_idx).unwrap_or("").trim().to_string()
        };

        let creation_date = get(3, 1);
        let system = get(1, 5);
        let bank = get(2, 1);
        let client_account = get(4, 12);
        let client_name = get(5, 12);
        let period_from = get(6, 2);
        let period_until = get(6, 15);
        let currency = get(7, 2);
        let last_transaction_date = get(7, 12);

        Ok(CsvHeader {
            creation_date,
            system,
            bank,
            client_account,
            client_name,
            period_from,
            period_until,
            currency,
            last_transaction_date,
        })
    }
}

/// Операция из CSV-выписки
#[derive(Debug, Default)]
pub(crate) struct CsvRecord {
    // дата проводки
    booking_date: String,
    debit_account: String,
    credit_account: String,
    debit_amount: Option<String>,
    credit_amount: Option<String>,
    doc_number: String,
    operation_type: String,
    bank: String,
    transaction_purpose: Option<String>,
}

impl CsvRecord {
    /// Распаковывает колонки из записи csv-файла в структуру
    fn from_string_record(row: &StringRecord, layout: &TableLayout) -> Self {
        let get = |idx: usize| -> String {
            row.get(idx)
                .unwrap_or_else(|| panic!("row does not match layout at index {idx}: {:?}", row))
                .trim()
                .to_string()
        };

        let booking_date = get(layout.booking_date_col);
        let debit_account = get(layout.debit_account_col);
        let credit_account = get(layout.credit_account_col);
        let debit_amount = row
            .get(layout.debit_amount_col)
            .map(|s| s.trim().to_string());

        let credit_amount = row
            .get(layout.credit_amount_col)
            .map(|s| s.trim().to_string());
        let doc_number = get(layout.doc_number_col);
        let operation_type = get(layout.operation_type_col);
        let bank = get(layout.bank_col);
        let transaction_purpose = row
            .get(layout.transaction_purpose_col)
            .map(|s| s.trim().to_string());

        CsvRecord {
            booking_date,
            debit_account,
            credit_account,
            debit_amount,
            credit_amount,
            doc_number,
            operation_type,
            bank,
            transaction_purpose,
        }
    }

    fn into_transaction(self, our_account: &str) -> Result<Transaction, ParseError> {
        let booking_date = NaiveDate::parse_from_str(&self.booking_date, "%d.%m.%Y")?;
        let value_date: Option<NaiveDate> = None;
        let (amount, direction) = parse_amount_and_direction(
            self.debit_amount.as_deref(),
            self.credit_amount.as_deref(),
        )?;
        let description = self.transaction_purpose.unwrap_or_default();
        let (counterparty, counterparty_name) =
            extract_counterparty_account(&self.debit_account, &self.credit_account, our_account);

        Ok(Transaction::new(
            booking_date,
            value_date,
            amount,
            direction,
            description,
            counterparty,
            counterparty_name,
        ))
    }
}

#[derive(Debug, Default)]
pub struct CsvFooter {
    opening_balance: Balance,
    closing_balance: Balance,
}

impl CsvFooter {
    fn from_string_records(rows: &[StringRecord]) -> Result<Self, ParseError> {
        let mut opening: Option<Balance> = None;
        let mut closing: Option<Balance> = None;

        for row in rows {
            let title = row.get(1).unwrap_or("").trim();

            match title {
                "Входящий остаток" => {
                    opening = Some(parse_footer_balance(row)?);
                }
                "Исходящий остаток" => {
                    closing = Some(parse_footer_balance(row)?);
                }
                _ => {}
            }
        }

        let opening_balance = opening
            .ok_or_else(|| ParseError::Header("opening balance not found in footer".into()))?;

        let closing_balance = closing
            .ok_or_else(|| ParseError::Header("closing balance not found in footer".into()))?;

        Ok(CsvFooter {
            opening_balance,
            closing_balance,
        })
    }
}

/// Индексы нужных колонок поимённо
///
/// Вспомогательная структура для хранения, в каких столбцах csv содержатся данные для нужного поля
struct TableLayout {
    booking_date_col: usize,
    debit_account_col: usize,
    credit_account_col: usize,
    debit_amount_col: usize,
    credit_amount_col: usize,
    doc_number_col: usize,
    operation_type_col: usize,
    bank_col: usize,
    transaction_purpose_col: usize,
}

impl TableLayout {
    /// По паттернам строк определяет индексы необходимых колонок
    fn from_string_records(
        headers_row: &StringRecord,
        subheaders_row: &StringRecord,
    ) -> Result<Self, ParseError> {
        // первая строка заголовков - основные
        let booking_date_col = find_col(headers_row, "Дата проводки")?;
        let debit_account_col = find_col(subheaders_row, "Дебет")?;
        let credit_account_col = find_col(subheaders_row, "Кредит")?;
        let doc_number_col = find_col(headers_row, "№ документа")?;
        let operation_type_col = find_col(headers_row, "ВО")?;
        let bank_col = find_col(headers_row, "Банк")?;
        let transaction_purpose_col = find_col(headers_row, "Назначение платежа")?;

        // вторая строка с подзаголовками: под «Сумма» стоят "Дебет" и "Кредит"
        let debit_amount_col = find_col(headers_row, "Сумма по дебету")?;
        let credit_amount_col = find_col(headers_row, "Сумма по кредиту")?;

        Ok(TableLayout {
            booking_date_col,
            debit_account_col,
            credit_account_col,
            debit_amount_col,
            credit_amount_col,
            doc_number_col,
            operation_type_col,
            bank_col,
            transaction_purpose_col,
        })
    }
}

/// Структура с сырыми данными формата CSV.
///
/// Для парсинга используйте [`CsvData::parse`].
///
/// Пример:
/// ```rust,no_run
/// use std::io::Cursor;
/// use parser::CsvData;
/// # use parser::ParseError;
/// # fn main() -> Result<(), ParseError> {
/// let reader = Cursor::new("date,amount\n2024-01-01,100\n");
/// let data = CsvData::parse(reader)?;
/// #     Ok(())
/// # }
/// ```
pub struct CsvData {
    header: CsvHeader,
    records: Vec<CsvRecord>,
    footer: CsvFooter,
}

impl TryFrom<CsvData> for Statement {
    type Error = ParseError;
    fn try_from(data: CsvData) -> Result<Self, Self::Error> {
        let account_id = data.header.client_account;
        let account_name = Some(data.header.client_name);
        let currency = parse_currency(&data.header.currency);
        let opening_balance: Option<Balance> = Some(data.footer.opening_balance);
        let closing_balance: Option<Balance> = Some(data.footer.closing_balance);
        let period_from = data
            .header
            .period_from
            .trim_start_matches("за период с")
            .trim();
        let period_until = data.header.period_until.trim_start_matches("по").trim();

        let period_from = parse_rus_date(period_from)?;
        let period_until = parse_rus_date(period_until)?;

        let transactions = data
            .records
            .into_iter()
            .map(|rec: CsvRecord| rec.into_transaction(&account_id))
            .collect::<Result<Vec<Transaction>, ParseError>>()?;

        Ok(Statement::new(
            account_id,
            account_name,
            currency,
            opening_balance,
            closing_balance,
            transactions,
            period_from,
            period_until,
        ))
    }
}

impl CsvData {
    /// Парсит при помощи переданного reader данные  в [`CsvData`]
    ///
    /// При ошибке возвращает [`ParseError`]
    pub fn parse<R: Read>(reader: R) -> Result<Self, ParseError> {
        let mut rdr = ReaderBuilder::new().has_headers(false).from_reader(reader);

        let mut header_rows: Vec<StringRecord> = Vec::new();
        let mut data_rows: Vec<StringRecord> = Vec::new();
        let mut footer_rows: Vec<StringRecord> = Vec::new();

        let mut in_data_section = false;

        // строки с заголовками
        let mut headers_row: Option<StringRecord> = None;
        let mut subheaders_row: Option<StringRecord> = None;

        let mut records_iter = rdr.records();

        // читаем сначала ряды заголовка выписки, потом ряды с операциями
        while let Some(result) = records_iter.next() {
            let record = result?;

            if !in_data_section {
                // если наткнулись на заголовки таблицы - значит, заголовок файла закончился
                if record.iter().any(|field| field.contains("Дата проводки")) {
                    headers_row = Some(record);
                    if let Some(next_result) = records_iter.next() {
                        let r = next_result?;
                        subheaders_row = Some(r);
                    } else {
                        return Err(ParseError::Header(
                            "unexpected EOF: second header row missing".into(),
                        ));
                    }

                    in_data_section = true;
                } else {
                    header_rows.push(record);
                }
            } else {
                // footer
                if is_footer_row(&record) {
                    footer_rows.push(record);

                    for result in records_iter {
                        footer_rows.push(result?);
                    }

                    break;
                } else {
                    data_rows.push(record);
                }
            }
        }

        let headers_row =
            headers_row.ok_or_else(|| ParseError::Header("table headers row not found".into()))?;
        let subheaders_row = subheaders_row
            .ok_or_else(|| ParseError::Header("table subheaders row not found".into()))?;

        if footer_rows.is_empty() {
            return Err(ParseError::Header("footer rows not found".into()));
        }

        let header = CsvHeader::from_string_records(&header_rows)?;
        let layout = TableLayout::from_string_records(&headers_row, &subheaders_row)?;

        let mut records = Vec::new();
        for row in data_rows {
            if row.iter().all(|f| f.trim().is_empty()) {
                continue;
            }

            let rec = CsvRecord::from_string_record(&row, &layout);
            records.push(rec);
        }

        let footer = CsvFooter::from_string_records(&footer_rows)?;

        Ok(CsvData {
            header,
            records,
            footer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Direction;
    use chrono::NaiveDate;
    use csv::StringRecord;

    // CsvHeader

    #[test]
    fn csv_header_from_string_records_extracts_fields() {
        // Ровно 8 строк, как ожидает CsvHeader::from_string_records

        // row 0 - не используется
        let row0 = {
            let v = vec![String::new(); 16];
            StringRecord::from(v)
        };

        // row 1 - system (col 5)
        let row1 = {
            let mut v = vec![String::new(); 16];
            v[5] = "СберБизнес. экспорт выписки".to_string();
            StringRecord::from(v)
        };

        // row 2 - bank (col 1)
        let row2 = {
            let mut v = vec![String::new(); 16];
            v[1] = "ПАО СБЕРБАНК".to_string();
            StringRecord::from(v)
        };

        // row 3 - creation_date (col 1)
        let row3 = {
            let mut v = vec![String::new(); 16];
            v[1] = "Дата формирования выписки 01.02.2023 в 10:20:30".to_string();
            StringRecord::from(v)
        };

        // row 4 - client_account (col 12)
        let row4 = {
            let mut v = vec![String::new(); 16];
            v[12] = "40702810OURACC".to_string();
            StringRecord::from(v)
        };

        // row 5 - client_name (col 12)
        let row5 = {
            let mut v = vec![String::new(); 16];
            v[12] = "ООО Ромашка".to_string();
            StringRecord::from(v)
        };

        // row 6 - period_from (col 2), period_until (col 15)
        let row6 = {
            let mut v = vec![String::new(); 16];
            v[2] = "за период с 01 января 2023 г.".to_string();
            v[15] = "по 31 января 2023 г.".to_string();
            StringRecord::from(v)
        };

        // row 7 - currency (col 2), last_transaction_date (col 12)
        let row7 = {
            let mut v = vec![String::new(); 16];
            v[2] = "RUB".to_string();
            v[12] = "Дата предыдущей операции по счету 31 января 2023 г.".to_string();
            StringRecord::from(v)
        };

        let rows = vec![row0, row1, row2, row3, row4, row5, row6, row7];

        let header = CsvHeader::from_string_records(&rows).expect("header parse must succeed");

        assert_eq!(
            header.creation_date,
            "Дата формирования выписки 01.02.2023 в 10:20:30"
        );
        assert_eq!(header.system, "СберБизнес. экспорт выписки");
        assert_eq!(header.bank, "ПАО СБЕРБАНК");
        assert_eq!(header.client_account, "40702810OURACC");
        assert_eq!(header.client_name, "ООО Ромашка");
        assert_eq!(header.period_from, "за период с 01 января 2023 г.");
        assert_eq!(header.period_until, "по 31 января 2023 г.");
        assert_eq!(header.currency, "RUB");
        assert_eq!(
            header.last_transaction_date,
            "Дата предыдущей операции по счету 31 января 2023 г."
        );
    }

    #[test]
    fn csv_header_errors_on_not_enough_rows() {
        let row0 = {
            let v = vec![String::new(); 4];
            StringRecord::from(v)
        };
        let row1 = {
            let v = vec![String::new(); 4];
            StringRecord::from(v)
        };

        let rows = vec![row0, row1];

        let err = CsvHeader::from_string_records(&rows).unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(msg.contains("not enough rows"), "unexpected msg: {msg}");
            }
            other => panic!("expected Header error, got {other:?}"),
        }
    }

    // TableLayout & CsvRecord

    #[test]
    fn table_layout_finds_expected_columns() {
        // Первая строка заголовков таблицы
        let headers_row = {
            let mut v = vec![String::new(); 7];
            v[0] = "Дата проводки".to_string();
            v[1] = "№ документа".to_string();
            v[2] = "ВО".to_string();
            v[3] = "Банк".to_string();
            v[4] = "Сумма по дебету".to_string();
            v[5] = "Сумма по кредиту".to_string();
            v[6] = "Назначение платежа".to_string();
            StringRecord::from(v)
        };

        // Вторая строка - подзаголовки
        let subheaders_row = {
            let mut v = vec![String::new(); 7];
            v[1] = "Дебет".to_string();
            v[2] = "Кредит".to_string();
            StringRecord::from(v)
        };

        let layout = TableLayout::from_string_records(&headers_row, &subheaders_row)
            .expect("layout must succeed");

        assert_eq!(layout.booking_date_col, 0);
        assert_eq!(layout.doc_number_col, 1);
        assert_eq!(layout.operation_type_col, 2);
        assert_eq!(layout.bank_col, 3);
        assert_eq!(layout.debit_amount_col, 4);
        assert_eq!(layout.credit_amount_col, 5);
        assert_eq!(layout.transaction_purpose_col, 6);
        assert_eq!(layout.debit_account_col, 1);
        assert_eq!(layout.credit_account_col, 2);
    }

    #[test]
    fn csv_record_from_string_record_extracts_trimmed_fields() {
        // layout из предыдущего теста
        let headers_row = {
            let mut v = vec![String::new(); 7];
            v[0] = "Дата проводки".to_string();
            v[1] = "№ документа".to_string();
            v[2] = "ВО".to_string();
            v[3] = "Банк".to_string();
            v[4] = "Сумма по дебету".to_string();
            v[5] = "Сумма по кредиту".to_string();
            v[6] = "Назначение платежа".to_string();
            StringRecord::from(v)
        };
        let subheaders_row = {
            let mut v = vec![String::new(); 7];
            v[1] = "Дебет".to_string();
            v[2] = "Кредит".to_string();
            StringRecord::from(v)
        };
        let layout = TableLayout::from_string_records(&headers_row, &subheaders_row)
            .expect("layout must succeed");

        let row = {
            let mut v = vec![String::new(); 7];
            v[0] = " 10.01.2023 ".to_string();
            v[1] = " 40702810OUR ".to_string(); // debit_account
            v[2] = " 40702810CP ".to_string(); // credit_account
            v[3] = " БАНК ".to_string();
            v[4] = " 123.45 ".to_string(); // debit_amount
            v[5] = "  ".to_string(); // empty credit_amount
            v[6] = "  Назначение  ".to_string();
            StringRecord::from(v)
        };

        let rec = CsvRecord::from_string_record(&row, &layout);

        assert_eq!(rec.booking_date, "10.01.2023");
        assert_eq!(rec.debit_account, "40702810OUR");
        assert_eq!(rec.credit_account, "40702810CP");
        assert_eq!(rec.debit_amount.as_deref(), Some("123.45"));
        assert_eq!(rec.credit_amount.as_deref(), Some(""));
        assert_eq!(rec.bank, "БАНК");
        assert_eq!(rec.transaction_purpose.as_deref(), Some("Назначение"));
    }

    #[test]
    fn csv_record_into_transaction_parses_amount_and_counterparty() {
        // layout
        let headers_row = {
            let mut v = vec![String::new(); 7];
            v[0] = "Дата проводки".to_string();
            v[1] = "№ документа".to_string();
            v[2] = "ВО".to_string();
            v[3] = "Банк".to_string();
            v[4] = "Сумма по дебету".to_string();
            v[5] = "Сумма по кредиту".to_string();
            v[6] = "Назначение платежа".to_string();
            StringRecord::from(v)
        };
        let subheaders_row = {
            let mut v = vec![String::new(); 7];
            v[1] = "Дебет".to_string();
            v[2] = "Кредит".to_string();
            StringRecord::from(v)
        };
        let layout = TableLayout::from_string_records(&headers_row, &subheaders_row)
            .expect("layout must succeed");

        // одна строка таблицы
        let row = {
            let mut v = vec![String::new(); 7];
            v[0] = "10.01.2023".to_string();
            v[1] = "OUR_ACC".to_string(); // debit_account
            v[2] = "CP_ACC".to_string(); // credit_account
            v[3] = "БАНК".to_string();
            v[4] = "100.00".to_string(); // debit_amount
            v[5] = "".to_string(); // credit_amount
            v[6] = "Платёж контрагенту".to_string();
            StringRecord::from(v)
        };

        let rec = CsvRecord::from_string_record(&row, &layout);
        let tx = rec
            .into_transaction("OUR_ACC")
            .expect("into_transaction must succeed");

        assert_eq!(
            tx.booking_date,
            NaiveDate::parse_from_str("10.01.2023", "%d.%m.%Y").unwrap()
        );
        assert_eq!(tx.direction, Direction::Debit);
        assert_eq!(tx.amount, 10_000);
        assert_eq!(tx.counterparty.as_deref(), Some("CP_ACC"));
        assert_eq!(tx.description, "Платёж контрагенту");
    }

    // CsvFooter

    #[test]
    fn csv_footer_parses_opening_and_closing_balances() {
        let opening_row = {
            let mut v = vec![String::new(); 21];
            v[1] = "Входящий остаток".to_string();
            v[11] = "100.00".to_string();
            StringRecord::from(v)
        };

        let closing_row = {
            let mut v = vec![String::new(); 21];
            v[1] = "Исходящий остаток".to_string();
            v[11] = "150.00".to_string();
            StringRecord::from(v)
        };

        let footer = CsvFooter::from_string_records(&[opening_row, closing_row])
            .expect("footer parse must succeed");

        assert_eq!(footer.opening_balance, 10_000);
        assert_eq!(footer.closing_balance, 15_000);
    }

    #[test]
    fn csv_footer_errors_if_balances_missing() {
        let row = {
            let mut v = vec![String::new(); 5];
            v[1] = "Что-то ещё".to_string();
            StringRecord::from(v)
        };

        let err = CsvFooter::from_string_records(&[row]).unwrap_err();
        match err {
            ParseError::Header(msg) => {
                assert!(
                    msg.contains("opening balance") || msg.contains("closing balance"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected Header error, got {other:?}"),
        }
    }
}
