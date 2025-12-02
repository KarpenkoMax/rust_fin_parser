mod utils;

use std::io::{Read};
use chrono::NaiveDate;
use csv::{ReaderBuilder, StringRecord};
use crate::error::ParseError;
use crate::model::{Balance, Statement, Transaction};
use crate::utils::parse_currency;
use utils::*;

impl From<csv::Error> for ParseError {
    fn from(e: csv::Error) -> Self {
        ParseError::Csv(e)
    }
}

/// Структура с данными из заголовка CSV-выписки
#[derive(Debug, Default)]
pub struct CsvHeader {
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
            rows[row_idx]
                .get(col_idx)
                .unwrap_or("")
                .trim()
                .to_string()
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
pub struct CsvRecord {
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
        let debit_amount = match row.get(layout.debit_amount_col) {
            Some(s) => Some(s.trim().to_string()),
            None => None,
        };

        let credit_amount = match row.get(layout.credit_amount_col) {
            Some(s) => Some(s.trim().to_string()),
            None => None,
        };
        let doc_number = get(layout.doc_number_col);
        let operation_type = get(layout.operation_type_col);
        let bank = get(layout.bank_col);
        let transaction_purpose = match row.get(layout.transaction_purpose_col) {
            Some(s) => Some(s.trim().to_string()),
            None => None,
        };

        CsvRecord{
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
            self.credit_amount.as_deref()
        )?;
        let description = self.transaction_purpose.unwrap_or_default();
        let (counterparty, counterparty_name) = extract_counterparty_account(
            &self.debit_account, &self.credit_account, our_account
        );

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

        let opening_balance = opening.ok_or_else(|| {
            ParseError::Header("opening balance not found in footer".into())
        })?;

        let closing_balance = closing.ok_or_else(|| {
            ParseError::Header("closing balance not found in footer".into())
        })?;

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
    fn from_string_records(headers_row: &StringRecord, subheaders_row: &StringRecord) -> Result<Self, ParseError> {
        // первая строка заголовков - основные
        let booking_date_col = find_col(headers_row, "Дата проводки")?;
        let debit_account_col  = find_col(subheaders_row, "Дебет")?;
        let credit_account_col = find_col(subheaders_row, "Кредит")?;
        let doc_number_col = find_col(headers_row, "№ документа")?;
        let operation_type_col = find_col(headers_row, "ВО")?;
        let bank_col = find_col(headers_row, "Банк")?;
        let transaction_purpose_col = find_col(headers_row, "Назначение платежа")?;

        // вторая строка с подзаголовками: под «Сумма» стоят "Дебет" и "Кредит"
        let debit_amount_col  = find_col(headers_row, "Сумма по дебету")?;
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
            transaction_purpose_col
        })
    }
}

pub struct CsvData {
    header: CsvHeader,
    records: Vec<CsvRecord>,
    footer: CsvFooter,
}

fn parse_rus_date(raw: &str) -> Result<NaiveDate, ParseError> {
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

impl TryFrom<CsvData> for Statement {
    type Error = ParseError;
    fn try_from(data: CsvData) -> Result<Self, Self::Error> {
        let account_id = data.header.client_account;
        let account_name = Some(data.header.client_name);
        let currency = parse_currency(&data.header.currency);
        let opening_balance: Option<Balance> = Some(data.footer.opening_balance);
        let closing_balance: Option<Balance> = Some(data.footer.closing_balance);
        let period_from = data.header.period_from.trim_start_matches("за период с").trim();
        let period_until = data.header.period_until.trim_start_matches("по").trim();

        let period_from = parse_rus_date(period_from)?;
        let period_until = parse_rus_date(period_until)?;

        let transactions = data.records
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
            period_until
        ))
    }
}

impl CsvData {
    pub fn parse<R: Read>(reader: R) -> Result<Self, ParseError> {
        let mut rdr = ReaderBuilder::new()
            .has_headers(false)
            .from_reader(reader);

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
                        return Err(ParseError::Header("unexpected EOF: second header row missing".into()));
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

        let headers_row = headers_row.ok_or_else(|| ParseError::Header("table headers row not found".into()))?;
        let subheaders_row = subheaders_row.ok_or_else(|| ParseError::Header("table subheaders row not found".into()))?;

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

        Ok(CsvData { header, records, footer })
    }
}

