pub(crate) mod serde_models;

use std::io::{BufRead, Write};
use chrono::{NaiveDate, Utc};
use serde::{Serialize, Deserialize};
use crate::error::ParseError;
use crate::model::{Balance, Currency, Direction, Statement, Transaction};
use quick_xml::de::{DeError, from_reader};
use serde_models::*; 
use crate::utils::{parse_currency, parse_signed_balance, parse_amount};
use quick_xml::se::{to_utf8_io_writer, SeError};


impl From<DeError> for ParseError {
    fn from(e: DeError) -> Self {
        ParseError::CamtDe(e)
    }
}

impl From<SeError> for ParseError {
    fn from(e: SeError) -> Self {
        ParseError::CamtSe(e)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Camt053Data {
    pub statement: Camt053Statement,
}

impl Camt053Data {
    pub fn parse<R: BufRead>(reader: R) -> Result<Self, ParseError> {
        let doc: Camt053Document = from_reader(reader)?;

        let mut stmt_iter = doc
            .bank_to_customer
            .statements
            .into_iter();

        // first statement only
        let stmt = stmt_iter
            .next()
            .ok_or_else(|| ParseError::BadInput("CAMT file has no <Stmt>".into()))?;

        if let Some(_) = stmt_iter.next() {
            eprintln!("more than one statement provided in camt053. only reading first");
        }

        Ok(Camt053Data { statement: stmt })
    }

    pub fn write<W: Write>(self, writer: W) -> Result<(), ParseError> {
        let doc = Camt053Document {
            bank_to_customer: Camt053BankToCustomer {
                group_header: Some(Camt053GroupHeader {
                    message_id: format!(
                        "serialized_via_parser-{}", Utc::now().format("%Y%m%d%H%M%S")
                    ),
                    created_at: Some(Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()),
                }),
                statements: vec![self.statement],
            }
        };

        to_utf8_io_writer(writer, &doc)?;
        Ok(())
    }
}

fn detect_currency(stmt: &Camt053Statement) -> Result<Currency, ParseError> {
    // Пробуем валюту счёта
    if let Some(ref ccy) = stmt.account.currency {
        return Ok(parse_currency(ccy));
    }

    // Пробуем валюту из балансa
    if let Some(bal_ccy) = stmt
        .balances
        .iter()
        .find_map(|bal| Some(bal.amount.currency.as_str()))
    {
        return Ok(parse_currency(bal_ccy));
    }

    // Пробуем валюту из первой операции
    if let Some(entry) = stmt.entries.first() {
        return Ok(parse_currency(&entry.amount.currency));
    }

    Err(ParseError::InvalidCurrency("no currency found".into()))
}

fn balance_from_camt(bal: &Camt053Balance) -> Result<Balance, ParseError> {
    let dir = match bal.cdt_dbt_ind.as_deref() {
        Some("CRDT") => Direction::Credit,
        Some("DBIT") => Direction::Debit,
        other => {
            return Err(ParseError::InvalidAmount(format!(
                "unknown CdtDbtInd: {:?}",
                other
            )));
        }
    };

    parse_signed_balance(&bal.amount.value, dir)
}

fn extract_balances(stmt: &Camt053Statement) -> (Option<Balance>, Option<Balance>) {
    let mut opening = None;
    let mut closing = None;

    for bal in &stmt.balances {
        let code = bal
            .balance_type
            .code_or_proprietary
            .code
            .as_deref();

        let parsed = balance_from_camt(&bal).ok();

        match code {
            Some("OPBD") => opening = parsed,
            Some("CLBD") => closing = parsed,
            _ => {}
        }
    }

    (opening, closing)
}

fn parse_camt_date_to_naive(s: &str) -> Result<NaiveDate, ParseError> {
    // CAMT может прислать "2023-04-20" или "2023-04-20T23:59:59"
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d);
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.date());
    }
    Err(ParseError::BadInput(format!("invalid CAMT date: {s}")))
}

fn detect_period(stmt: &Camt053Statement) -> Result<(NaiveDate, NaiveDate), ParseError> {
    // Пытаемся извлечь из FrToDt 
    if let Some(period) = &stmt.period {
        if let (Some(raw_from), Some(raw_to)) = (&period.from, &period.to) {
            let from = parse_camt_date_to_naive(raw_from)?;
            let to = parse_camt_date_to_naive(raw_to)?;

            return Ok((from, to));
        }
    }
    // не получилось - идём искать min/max из транзакций
    let mut min_date: Option<NaiveDate> = None;
    let mut max_date: Option<NaiveDate> = None;

    for entry in &stmt.entries {
        let d = parse_camt_date_to_naive(&entry.booking_date.date)?;

        min_date = Some(match min_date {
            Some(cur) => cur.min(d),
            None => d            
        });

        max_date = Some(match max_date {
            Some(cur) => cur.max(d),
            None => d            
        });
    }

    match (min_date, max_date) {
        (Some(from), Some(to)) => Ok((from, to)),
        _ => Err(ParseError::BadInput("missing camt statement period".into()))   
    }
}

fn counterparty_from_tx(
    tx: &CamtTxDtls,
    direction: Direction,
) -> (Option<String>, Option<String>) {
    let parties = match &tx.related_parties {
        Some(p) => p,
        None => return (None, None),
    };

    // Выбираем "персону" контрагента: сначала Ultmt*, если есть, иначе обычный
    let party_opt = match direction {
        Direction::Debit => {
            parties
                .ultimate_creditor
                .as_ref()
                .or(parties.creditor.as_ref())
        }
        Direction::Credit => {
            parties
                .ultimate_debtor
                .as_ref()
                .or(parties.debtor.as_ref())
        }
    };

    let counterparty_name = party_opt.and_then(|p| p.name.clone());

    // Счёт контрагента (IBAN)
    let account_opt = match direction {
        Direction::Debit => parties.creditor_account.as_ref(),
        Direction::Credit => parties.debtor_account.as_ref(),
    };

    let counterparty_id = account_opt
        .and_then(|acc| acc.id.iban.clone());

    (counterparty_id, counterparty_name)
}

fn description_from_tx(tx: &CamtTxDtls) -> String {
    if let Some(rmt) = &tx.rmt_inf {
        if !rmt.unstructured.is_empty() {
            return rmt.unstructured.join("\n");
        }
    }
    String::new()
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
                    "unknown CdtDbtInd: {other}"
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
        let account_id = data
            .statement
            .account
            .id
            .iban
            .clone()
            .unwrap_or_else(|| "not provided".to_string());

        let account_name = data.statement.account.name.clone();

        let currency = detect_currency(&data.statement)?;
        let (opening_balance, closing_balance) = extract_balances(&data.statement);
        let (period_from, period_until) = detect_period(&data.statement)?;

        let transactions: Vec<Transaction> = data.statement
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