use super::common;

use crate::model::{Balance, Currency, Direction, Statement, Transaction};
use chrono::NaiveDate;

use crate::camt053::serde_models::*;

/// ISO-код валюты для CAMT (ISO 4217).
pub(super) fn currency_code(cur: &Currency) -> &'static str {
    match cur {
        Currency::RUB => "RUB",
        Currency::EUR => "EUR",
        Currency::USD => "USD",
        Currency::CNY => "CNY",
        Currency::Other(c) => {
            println!(
                "found unknown currency {c} while converting to camt053. using placeholder '???'"
            );
            "???"
        }
    }
}

pub(super) fn format_iso_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

/// Балансы (OPBD / CLBD)
pub(super) fn balances_from_statement(stmt: &Statement, ccy_code: &str) -> Vec<Camt053Balance> {
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
        ("CRDT".to_string(), common::format_minor_units(value, '.'))
    } else {
        ("DBIT".to_string(), common::format_minor_units(-value, '.'))
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

///  Преобразует транзакции в Ntry
pub(super) fn entries_from_transactions(txs: &[Transaction], ccy_code: &str) -> Vec<Camt053Entry> {
    txs.iter()
        .map(|tx| entry_from_transaction(tx, ccy_code))
        .collect()
}

pub(super) fn entry_from_transaction(tx: &Transaction, ccy_code: &str) -> Camt053Entry {
    let cdt_dbt_ind = match tx.direction {
        Direction::Credit => "CRDT".to_string(),
        Direction::Debit => "DBIT".to_string(),
    };

    // amount: u64 - считаем, что это "копейки"
    let amount_str = common::format_minor_units(tx.amount, '.');

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

    // RltdPties - контрагент: учитываем и счёт, и имя
    let related_parties = {
        let cp_acc = tx.counterparty.as_ref();
        let cp_name = tx.counterparty_name.clone();

        // если нет ни счёта, ни имени - не пишем RltdPties вообще
        if cp_acc.is_none() && cp_name.is_none() {
            None
        } else {
            let party = CamtParty {
                name: cp_name,
                postal_address: None,
                id: None,
            };

            let account_opt = cp_acc.map(|acc| CamtAccount {
                id: CamtAccountId {
                    iban: Some(acc.clone()),
                },
            });

            Some(match tx.direction {
                // Нам пришли деньги: контрагент - дебитор
                Direction::Credit => CamtRelatedParties {
                    debtor: Some(party),
                    debtor_account: account_opt,
                    creditor: None,
                    creditor_account: None,
                    ultimate_debtor: None,
                    ultimate_creditor: None,
                },
                // Мы платим: контрагент - кредитор
                Direction::Debit => CamtRelatedParties {
                    debtor: None,
                    debtor_account: None,
                    creditor: Some(party),
                    creditor_account: account_opt,
                    ultimate_debtor: None,
                    ultimate_creditor: None,
                },
            })
        }
    };

    let tx_dtls = CamtTxDtls {
        refs: None,
        amount_details: None,
        related_parties,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Currency, Direction, Statement, Transaction};
    use chrono::NaiveDate;

    fn d(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn currency_code_returns_iso_for_known_currencies() {
        assert_eq!(currency_code(&Currency::RUB), "RUB");
        assert_eq!(currency_code(&Currency::EUR), "EUR");
        assert_eq!(currency_code(&Currency::USD), "USD");
        assert_eq!(currency_code(&Currency::CNY), "CNY");
    }

    #[test]
    fn currency_code_returns_placeholder_for_other() {
        let cur = Currency::Other("ABC".to_string());
        assert_eq!(currency_code(&cur), "???");
    }

    #[test]
    fn format_iso_date_formats_correctly() {
        let date = d(2023, 4, 19);
        assert_eq!(format_iso_date(date), "2023-04-19");

        let date = d(1999, 12, 31);
        assert_eq!(format_iso_date(date), "1999-12-31");
    }

    #[test]
    fn balances_from_statement_creates_opening_and_closing_balances() {
        let stmt = Statement::new(
            "ACC".to_string(),
            Some("Test account".to_string()),
            Currency::EUR,
            Some(100_00),
            Some(-50_00),
            Vec::new(),
            d(2023, 1, 1),
            d(2023, 1, 31),
        );

        let balances = balances_from_statement(&stmt, "EUR");
        assert_eq!(balances.len(), 2);

        let opbd = &balances[0];
        assert_eq!(
            opbd.balance_type.code_or_proprietary.code.as_deref(),
            Some("OPBD")
        );
        assert_eq!(opbd.amount.currency, "EUR");
        assert_eq!(opbd.amount.value, "100.00");
        assert_eq!(opbd.cdt_dbt_ind.as_deref(), Some("CRDT"));

        let clbd = &balances[1];
        assert_eq!(
            clbd.balance_type.code_or_proprietary.code.as_deref(),
            Some("CLBD")
        );
        assert_eq!(clbd.amount.currency, "EUR");
        assert_eq!(clbd.amount.value, "50.00");
        assert_eq!(clbd.cdt_dbt_ind.as_deref(), Some("DBIT"));
    }

    #[test]
    fn balances_from_statement_skips_absent_balances() {
        let stmt = Statement::new(
            "ACC".to_string(),
            None,
            Currency::EUR,
            None,
            None,
            Vec::new(),
            d(2023, 1, 1),
            d(2023, 1, 31),
        );

        let balances = balances_from_statement(&stmt, "EUR");
        assert!(balances.is_empty());
    }

    #[test]
    fn entry_from_transaction_credit_with_description() {
        let tx = Transaction::new(
            d(2023, 4, 19),
            None,
            12345,
            Direction::Credit,
            "Test payment".to_string(),
            None,
            None,
        );

        let entry = entry_from_transaction(&tx, "EUR");

        assert_eq!(entry.amount.currency, "EUR");
        assert_eq!(entry.amount.value, "123.45");
        assert_eq!(entry.cdt_dbt_ind, "CRDT");

        assert_eq!(entry.booking_date.date, "2023-04-19");
        // value_date = booking_date, т.к. value_date == None
        assert_eq!(entry.value_date.date, "2023-04-19");

        // проверяем, что описание попало в RmtInf/Ustrd
        let details = entry.details.expect("details must be present");
        assert_eq!(details.tx_details.len(), 1);

        let tx_dtls = &details.tx_details[0];
        let rmt_inf = tx_dtls.rmt_inf.as_ref().expect("rmt_inf must be present");
        assert_eq!(rmt_inf.unstructured, vec!["Test payment".to_string()]);
    }

    #[test]
    fn entry_from_transaction_debit_without_description() {
        let tx = Transaction::new(
            d(2023, 4, 20),
            Some(d(2023, 4, 21)),
            500,
            Direction::Debit,
            "".to_string(),
            None,
            None,
        );

        let entry = entry_from_transaction(&tx, "RUB");

        assert_eq!(entry.amount.currency, "RUB");
        assert_eq!(entry.amount.value, "5.00");
        assert_eq!(entry.cdt_dbt_ind, "DBIT");

        assert_eq!(entry.booking_date.date, "2023-04-20");
        assert_eq!(entry.value_date.date, "2023-04-21");

        // если description пустой, RmtInf не создаётся
        let details = entry.details.expect("details must be present");
        assert_eq!(details.tx_details.len(), 1);
        let tx_dtls = &details.tx_details[0];
        assert!(tx_dtls.rmt_inf.is_none());
    }

    #[test]
    fn entries_from_transactions_maps_all_transactions() {
        let tx1 = Transaction::new(
            d(2023, 1, 10),
            None,
            10000,
            Direction::Credit,
            "First".to_string(),
            None,
            None,
        );

        let tx2 = Transaction::new(
            d(2023, 1, 11),
            None,
            2500,
            Direction::Debit,
            "Second".to_string(),
            None,
            None,
        );

        let entries = entries_from_transactions(&[tx1, tx2], "EUR");
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].amount.value, "100.00");
        assert_eq!(entries[0].cdt_dbt_ind, "CRDT");

        assert_eq!(entries[1].amount.value, "25.00");
        assert_eq!(entries[1].cdt_dbt_ind, "DBIT");
    }
}
