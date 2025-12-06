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

#[derive(Debug, Serialize, Deserialize)]
pub struct Camt053Data {
    pub statement: Camt053Statement,
}

impl Camt053Data {
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