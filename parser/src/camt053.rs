pub(crate) mod serde_models;
mod utils;

use std::io::{Read, BufReader};
use serde::{Serialize, Deserialize};
use crate::error::ParseError;
use crate::model::{Direction, Statement, Transaction};
use quick_xml::de::{DeError, from_str};
use serde_models::*; 
use crate::utils::{parse_amount};
use quick_xml::se::SeError;
use utils::*;


impl From<DeError> for ParseError {
    fn from(e: DeError) -> Self {
        ParseError::XmlDe(e)
    }
}

impl From<SeError> for ParseError {
    fn from(e: SeError) -> Self {
        ParseError::XmlSe(e)
    }
}

/// Структура с сырыми данными формата camt053 после первичной сериализации.
/// 
/// Для парсинга используйте [`Camt053Data::parse`].
/// 
/// Пример:
/// ```rust,no_run
/// use std::io::Cursor;
/// use parser::Camt053Data;
/// # use parser::ParseError;
/// # fn main() -> Result<(), ParseError> {
/// let xml = r#"<Document>...</Document>"#;
/// let reader = Cursor::new(xml.as_bytes());
/// let data = Camt053Data::parse(reader)?;
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct Camt053Data {
    pub(crate) statement: Camt053Statement,
}

impl Camt053Data {
    /// Парсит при помощи переданного reader данные  в [`Camt053Data`]
    /// 
    /// При ошибке возвращает [`ParseError`]
    pub fn parse<R: Read>(reader: R) -> Result<Self, ParseError> {

        let mut buf_reader = BufReader::new(reader);
        let mut xml = String::new();
        buf_reader.read_to_string(&mut xml)?;

        // чистим неразрывные пробелы
        let xml = xml.replace('\u{00A0}', " ");

        // пытаемся читать как полноценный <Document>
        if let Ok(doc) = from_str::<Camt053Document>(&xml) {
            let mut stmt_iter = doc.bank_to_customer.statements.into_iter();

            let stmt = stmt_iter
                .next()
                .ok_or_else(|| ParseError::BadInput("CAMT file has no <Stmt>".into()))?;

            if stmt_iter.next().is_some() {
                eprintln!("more than one statement provided to camt053 parser. only reading first");
            }

            return Ok(Camt053Data { statement: stmt });
        }

        // если не вышло - пробуем как <Stmt>
        let stmt: Camt053Statement = from_str(&xml)?;
        Ok(Camt053Data { statement: stmt })
    }
}

impl TryFrom<&Camt053Entry> for Transaction {
    type Error = ParseError;

    fn try_from(entry: &Camt053Entry) -> Result<Self, Self::Error> {
        // direction
        let direction = match entry.cdt_dbt_ind.as_str() {
            "CRDT" => Direction::Credit,
            "DBIT" => Direction::Debit,
            other => {
                return Err(ParseError::InvalidAmount(format!(
                    "unknown direction (CdtDbtInd): {other}"
                )));
            }
        };

        let amount = parse_amount(&entry.amount.value)?;
        let booking_date = parse_camt_date_to_naive(&entry.booking_date.date)?;
        let value_date = Some(parse_camt_date_to_naive(&entry.value_date.date)?);

        let tx_dtls = entry
            .details
            .as_ref()
            .and_then(|d| d.tx_details.first());

        let counterparty: Option<String>;
        let counterparty_name: Option<String>;
        let description: String;

        if let Some(tx_details) = tx_dtls {
            (counterparty, counterparty_name) = counterparty_from_tx(tx_details, direction);
            description = description_from_tx(tx_details);
        } else {
            (counterparty, counterparty_name) = (None, None);
            description = "".to_string();
        }

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

impl TryFrom<Camt053Data> for Statement {
    type Error = ParseError;

    fn try_from(data: Camt053Data) -> Result<Self, Self::Error> {
        Ok(
            Statement::try_from(data.statement)?
        )
    }
}

impl TryFrom<Camt053Statement> for Statement {
    type Error = ParseError;
    fn try_from(statement: Camt053Statement) -> Result<Self, Self::Error> {
        let account_id = statement
            .account
            .id
            .iban
            .clone()
            .unwrap_or_else(|| "not provided".to_string());

        let account_name = statement.account.name.clone();

        let currency = detect_currency(&statement)?;
        let (opening_balance, closing_balance) = extract_balances(&statement);
        let (period_from, period_until) = detect_period(&statement)?;

        let transactions: Vec<Transaction> = statement
            .entries
            .iter()
            .map(|e| e.try_into())
            .collect::<Result<_, ParseError>>()?;

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


#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Currency};
    use chrono::NaiveDate;
    use std::io::Cursor;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    // Camt053Data::parse

    #[test]
    fn parse_full_document_with_single_stmt() {
        let xml = r#"
        <Document>
          <BkToCstmrStmt>
            <Stmt>
              <Acct>
                <Id>
                  <IBAN>DE1234567890</IBAN>
                </Id>
                <Nm>Test Account</Nm>
                <Ccy>EUR</Ccy>
              </Acct>
              <Bal>
                <Tp>
                  <CdOrPrtry>
                    <Cd>OPBD</Cd>
                  </CdOrPrtry>
                </Tp>
                <Amt Ccy="EUR">100.00</Amt>
                <CdtDbtInd>CRDT</CdtDbtInd>
                <Dt>
                  <Dt>2023-01-01</Dt>
                </Dt>
              </Bal>
              <FrToDt>
                <FrDtTm>2023-01-01T00:00:00</FrDtTm>
                <ToDtTm>2023-01-31T00:00:00</ToDtTm>
              </FrToDt>
            </Stmt>
          </BkToCstmrStmt>
        </Document>
        "#;

        let cursor = Cursor::new(xml.as_bytes());
        let data = Camt053Data::parse(cursor).expect("parse must succeed");

        // Проверяем, что прочитан именно Stmt внутри Document
        let stmt = data.statement;
        assert_eq!(
            stmt.account.id.iban.as_deref(),
            Some("DE1234567890")
        );
        assert_eq!(stmt.account.name.as_deref(), Some("Test Account"));
        assert_eq!(stmt.account.currency.as_deref(), Some("EUR"));
    }

    #[test]
    fn parse_root_stmt_without_document() {
        let xml = r#"
        <Stmt>
          <Acct>
            <Id>
              <IBAN>DE0000000000</IBAN>
            </Id>
            <Nm>Root Stmt</Nm>
            <Ccy>USD</Ccy>
          </Acct>
        </Stmt>
        "#;

        let cursor = Cursor::new(xml.as_bytes());
        let data = Camt053Data::parse(cursor).expect("parse must succeed");

        assert_eq!(
            data.statement.account.id.iban.as_deref(),
            Some("DE0000000000")
        );
        assert_eq!(data.statement.account.name.as_deref(), Some("Root Stmt"));
        assert_eq!(data.statement.account.currency.as_deref(), Some("USD"));
    }

    #[test]
    fn parse_document_without_stmt_returns_error() {
        let xml = r#"
        <Document>
          <BkToCstmrStmt>
            <!-- нет Stmt -->
          </BkToCstmrStmt>
        </Document>
        "#;

        let cursor = Cursor::new(xml.as_bytes());
        let err = Camt053Data::parse(cursor).unwrap_err();

        // Должен быть BadInput с текстом про отсутствие Stmt
        match err {
            ParseError::BadInput(msg) => {
                assert!(
                    msg.contains("no <Stmt>"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected BadInput, got {other:?}"),
        }
    }

    // TryFrom<&Camt053Entry> for Transaction

    fn make_simple_entry(cdt_dbt: &str) -> Camt053Entry {
        Camt053Entry {
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "123.45".to_string(),
            },
            cdt_dbt_ind: cdt_dbt.to_string(),
            booking_date: CamtDateXml {
                date: "2023-01-10".to_string(),
            },
            value_date: CamtDateXml {
                date: "2023-01-11".to_string(),
            },
            details: None,
        }
    }

    #[test]
    fn entry_to_transaction_credit() {
        let entry = make_simple_entry("CRDT");

        let tx = Transaction::try_from(&entry).expect("conversion must succeed");

        assert_eq!(tx.direction, Direction::Credit);
        assert_eq!(tx.amount, 12345); // 123.45 → 12345
        assert_eq!(tx.booking_date, d(2023, 1, 10));
        assert_eq!(tx.value_date, Some(d(2023, 1, 11)));

        // без details - пустое описание и нет контрагентов
        assert_eq!(tx.description, "");
        assert!(tx.counterparty.is_none());
        assert!(tx.counterparty_name.is_none());
    }

    #[test]
    fn entry_to_transaction_debit() {
        let entry = make_simple_entry("DBIT");

        let tx = Transaction::try_from(&entry).expect("conversion must succeed");

        assert_eq!(tx.direction, Direction::Debit);
        assert_eq!(tx.amount, 12345);
    }

    #[test]
    fn entry_with_unknown_direction_returns_error() {
        let mut entry = make_simple_entry("CRDT");
        entry.cdt_dbt_ind = "WTF".to_string();

        let err = Transaction::try_from(&entry).unwrap_err();
        match err {
            ParseError::InvalidAmount(msg) => {
                assert!(
                    msg.contains("unknown direction"),
                    "unexpected message: {msg}"
                );
            }
            other => panic!("expected InvalidAmount, got {other:?}"),
        }
    }

    // TryFrom<Camt053Statement> / Camt053Data for Statement

    fn sample_camt_statement() -> Camt053Statement {
        // Один entry, чтобы была хотя бы 1 транзакция
        let entry = Camt053Entry {
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "10.00".to_string(),
            },
            cdt_dbt_ind: "CRDT".to_string(),
            booking_date: CamtDateXml {
                date: "2023-01-05".to_string(),
            },
            value_date: CamtDateXml {
                date: "2023-01-06".to_string(),
            },
            details: None,
        };

        Camt053Statement {
            id: Some("STMTID".to_string()),
            sequence_number: Some(1),
            created_at: None,
            period: None, // допустим, detect_period сам разберется по Ntry
            account: Camt053Account {
                id: Camt053AccountId {
                    iban: Some("DE1111222233334444".to_string()),
                },
                name: Some("Sample Account".to_string()),
                currency: Some("EUR".to_string()),
            },
            balances: Vec::new(),
            entries: vec![entry],
        }
    }

    #[test]
    fn statement_from_camt_statement_maps_basic_fields() {
        let camt_stmt = sample_camt_statement();

        let stmt = Statement::try_from(camt_stmt).expect("conversion must succeed");

        assert_eq!(stmt.account_id, "DE1111222233334444");
        assert_eq!(stmt.account_name.as_deref(), Some("Sample Account"));
        assert_eq!(stmt.currency, Currency::EUR);

        assert_eq!(stmt.transactions.len(), 1);
        let tx = &stmt.transactions[0];

        assert_eq!(tx.direction, Direction::Credit);
        assert_eq!(tx.amount, 1000);
        assert_eq!(tx.booking_date, d(2023, 1, 5));
        assert_eq!(tx.value_date, Some(d(2023, 1, 6)));
    }

    #[test]
    fn statement_from_camt_data_uses_inner_statement() {
        let camt_stmt = sample_camt_statement();
        let data = Camt053Data { statement: camt_stmt };

        let stmt = Statement::try_from(data).expect("conversion must succeed");

        assert_eq!(stmt.account_id, "DE1111222233334444");
        assert_eq!(stmt.transactions.len(), 1);
    }

    #[test]
    fn statement_from_camt_statement_uses_not_provided_when_no_iban() {
        let mut camt_stmt = sample_camt_statement();
        camt_stmt.account.id.iban = None;

        let stmt = Statement::try_from(camt_stmt).expect("conversion must succeed");

        assert_eq!(stmt.account_id, "not provided");
    }
}

