use chrono::NaiveDate;
use crate::error::ParseError;
use crate::model::{Balance, Currency, Direction};
use crate::utils::{parse_currency, parse_signed_balance};
use super::serde_models::*;

pub(crate) fn detect_currency(stmt: &Camt053Statement) -> Result<Currency, ParseError> {
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

pub(crate) fn balance_from_camt(bal: &Camt053Balance) -> Result<Balance, ParseError> {
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

pub(crate) fn extract_balances(stmt: &Camt053Statement) -> (Option<Balance>, Option<Balance>) {
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

pub(crate) fn parse_camt_date_to_naive(s: &str) -> Result<NaiveDate, ParseError> {
    // CAMT может прислать "2023-04-20" или "2023-04-20T23:59:59"
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d);
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.date());
    }
    Err(ParseError::BadInput(format!("invalid CAMT date: {s}")))
}

pub(crate) fn detect_period(stmt: &Camt053Statement) -> Result<(NaiveDate, NaiveDate), ParseError> {
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

pub(crate) fn counterparty_from_tx(
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

pub(crate) fn description_from_tx(tx: &CamtTxDtls) -> String {
    if let Some(rmt) = &tx.rmt_inf {
        if !rmt.unstructured.is_empty() {
            return rmt.unstructured.join("\n");
        }
    }
    String::new()
}