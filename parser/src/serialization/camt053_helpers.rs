use chrono::NaiveDate;
use crate::model::{Statement, Transaction, Direction, Currency, Balance};

use crate::camt053::serde_models::*;


/// ISO-код валюты для CAMT (ISO 4217).
pub(crate) fn currency_code(cur: &Currency) -> &'static str {
    match cur {
        Currency::RUB => "RUB",
        Currency::EUR => "EUR",
        Currency::USD => "USD",
        Currency::CNY => "CNY",
        Currency::Other(_) => "???",
    }
}

pub(crate) fn format_iso_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// CAMT-формат суммы: "1234.56".
pub(crate) fn format_camt_amount_minor(v: i128) -> String {
    let v = v.abs() as u128;
    let units = v / 100;
    let frac = v % 100;
    format!("{units}.{frac:02}")
}

/// Балансы (OPBD / CLBD)
pub(crate) fn balances_from_statement(stmt: &Statement, ccy_code: &str) -> Vec<Camt053Balance> {
    let mut result = Vec::new();

    if let Some(open) = stmt.opening_balance {
        result.push(make_balance("OPBD", open, ccy_code));
    }

    if let Some(close) = stmt.closing_balance {
        result.push(make_balance("CLBD", close, ccy_code));
    }

    result
}

fn make_balance(code: &str, value: Balance, ccy_code: &str) -> Camt053Balance {
    let (cdt_dbt_ind, amount_str) = if value >= 0 {
        ("CRDT".to_string(), format_camt_amount_minor(value))
    } else {
        ("DBIT".to_string(), format_camt_amount_minor(-value))
    };

    Camt053Balance {
        balance_type: Camt053BalanceType {
            code_or_proprietary: Camt053BalanceCodeOrProprietary {
                code: Some(code.to_string()),
            },
        },
        amount: CamtAmtXml {
            currency: ccy_code.to_string(),
            value: amount_str,
        },
        cdt_dbt_ind: Some(cdt_dbt_ind),
        date: None,
    }
}

// ---------- Операции -> Ntry ----------

pub(crate) fn entries_from_transactions(
    txs: &[Transaction],
    ccy_code: &str,
) -> Vec<Camt053Entry> {
    txs.iter()
        .map(|tx| entry_from_transaction(tx, ccy_code))
        .collect()
}

pub(crate) fn entry_from_transaction(tx: &Transaction, ccy_code: &str) -> Camt053Entry {
    let cdt_dbt_ind = match tx.direction {
        Direction::Credit => "CRDT".to_string(),
        Direction::Debit => "DBIT".to_string(),
    };

    // amount: u64 - считаем, что это "копейки"
    let amount_str = format_camt_amount_minor(tx.amount as i128);

    let booking_date = CamtDateXml {
        date: format_iso_date(tx.booking_date),
    };

    let value_date = CamtDateXml {
        date: format_iso_date(tx.value_date.unwrap_or(tx.booking_date)),
    };

    // RmtInf / Ustrd - описание операции
    let rmt_inf = if tx.description.is_empty() {
        None
    } else {
        Some(CamtRemittanceInfo {
            unstructured: vec![tx.description.clone()],
            structured: Vec::new(),
        })
    };

    let tx_dtls = CamtTxDtls {
        refs: None,
        amount_details: None,
        related_parties: None,
        rmt_inf,
        related_datetimes: None,
    };

    let details = CamtEntryDetails {
        tx_details: vec![tx_dtls],
    };

    Camt053Entry {
        amount: CamtAmtXml {
            currency: ccy_code.to_string(),
            value: amount_str,
        },
        cdt_dbt_ind,
        booking_date,
        value_date,
        details: Some(details),
    }
}
