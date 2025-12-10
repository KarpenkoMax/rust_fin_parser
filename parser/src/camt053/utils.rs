use chrono::NaiveDate;
use crate::error::ParseError;
use crate::model::{Balance, Currency, Direction};
use crate::utils::{parse_currency, parse_signed_balance};
use super::serde_models::*;

pub(super) fn detect_currency(stmt: &Camt053Statement) -> Result<Currency, ParseError> {
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

pub(super) fn balance_from_camt(bal: &Camt053Balance) -> Result<Balance, ParseError> {
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

pub(super) fn extract_balances(stmt: &Camt053Statement) -> (Option<Balance>, Option<Balance>) {
    let mut opening = None;
    let mut closing = None;

    for bal in &stmt.balances {
        let code = bal
            .balance_type
            .code_or_proprietary
            .code
            .as_deref();

        let parsed = balance_from_camt(bal).ok();

        match code {
            Some("OPBD") => opening = parsed,
            Some("CLBD") => closing = parsed,
            _ => {}
        }
    }

    (opening, closing)
}

pub(super) fn parse_camt_date_to_naive(s: &str) -> Result<NaiveDate, ParseError> {
    // CAMT может прислать "2023-04-20" или "2023-04-20T23:59:59"
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d);
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Ok(dt.date());
    }
    Err(ParseError::BadInput(format!("invalid CAMT date: {s}")))
}

pub(super) fn detect_period(stmt: &Camt053Statement) -> Result<(NaiveDate, NaiveDate), ParseError> {
    // Пытаемся извлечь из FrToDt 
    if let Some(period) = &stmt.period
        && let (Some(raw_from), Some(raw_to)) = (&period.from, &period.to) {
            let from = parse_camt_date_to_naive(raw_from)?;
            let to = parse_camt_date_to_naive(raw_to)?;

            return Ok((from, to));
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

pub(super) fn counterparty_from_tx(
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

pub(super) fn description_from_tx(tx: &CamtTxDtls) -> String {
    if let Some(rmt) = &tx.rmt_inf
        && !rmt.unstructured.is_empty() {
            return rmt.unstructured.join("\n");
        }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Currency, Direction};

    fn empty_statement() -> Camt053Statement {
        Camt053Statement {
            account: Camt053Account {
                id: Camt053AccountId { iban: None },
                name: None,
                currency: None,
            },
            ..Default::default()
        }
    }

    // detect_currency

    #[test]
    fn detect_currency_prefers_account_currency() {
        let mut stmt = empty_statement();
        stmt.account.currency = Some("EUR".to_string());

        let ccy = detect_currency(&stmt).unwrap();
        assert_eq!(ccy, Currency::EUR);
    }

    #[test]
    fn detect_currency_uses_balance_if_no_account_currency() {
        let mut stmt = empty_statement();

        let bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary { code: None },
            },
            amount: CamtAmtXml {
                currency: "USD".to_string(),
                value: "100.00".to_string(),
            },
            cdt_dbt_ind: Some("CRDT".to_string()),
            date: None,
        };

        stmt.balances.push(bal);

        let ccy = detect_currency(&stmt).unwrap();
        assert_eq!(ccy, Currency::USD);
    }

    #[test]
    fn detect_currency_uses_entry_if_no_account_and_balances() {
        let mut stmt = empty_statement();

        let entry = Camt053Entry {
            amount: CamtAmtXml {
                currency: "CNY".to_string(),
                value: "50.00".to_string(),
            },
            ..Default::default()
        };

        stmt.entries.push(entry);

        let ccy = detect_currency(&stmt).unwrap();
        assert_eq!(ccy, Currency::CNY);
    }

    #[test]
    fn detect_currency_fails_if_no_sources() {
        let stmt = empty_statement();
        let err = detect_currency(&stmt).unwrap_err();

        match err {
            ParseError::InvalidCurrency(msg) => {
                assert!(msg.contains("no currency found"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // balance_from_camt

    #[test]
    fn balance_from_camt_parses_credit_as_positive() {
        let bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary { code: None },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "123.45".to_string(),
            },
            cdt_dbt_ind: Some("CRDT".to_string()),
            date: None,
        };

        let value = balance_from_camt(&bal).unwrap();
        assert!(value > 0, "credit balance should be positive, got {value}");
    }

    #[test]
    fn balance_from_camt_parses_debit_as_negative() {
        let bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary { code: None },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "123.45".to_string(),
            },
            cdt_dbt_ind: Some("DBIT".to_string()),
            date: None,
        };

        let value = balance_from_camt(&bal).unwrap();
        assert!(value < 0, "debit balance should be negative, got {value}");
    }

    #[test]
    fn balance_from_camt_fails_on_unknown_direction() {
        let bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary { code: None },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "10.00".to_string(),
            },
            cdt_dbt_ind: Some("SOMETHING".to_string()),
            date: None,
        };

        let err = balance_from_camt(&bal).unwrap_err();
        match err {
            ParseError::InvalidAmount(msg) => {
                assert!(msg.contains("unknown CdtDbtInd"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn balance_from_camt_credit_exact_minor_units() {
        let bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary { code: None },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "123.45".to_string(),
            },
            cdt_dbt_ind: Some("CRDT".to_string()),
            date: None,
        };

        let value = balance_from_camt(&bal).unwrap();
        assert_eq!(value, 12_345);
    }

    #[test]
    fn balance_from_camt_debit_exact_minor_units() {
        let bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary { code: None },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "987.65".to_string(),
            },
            cdt_dbt_ind: Some("DBIT".to_string()),
            date: None,
        };

        let value = balance_from_camt(&bal).unwrap();
        assert_eq!(value, -98_765);
    }

    // extract_balances

    #[test]
    fn extract_balances_selects_opening_and_closing_by_code() {
        let mut stmt = empty_statement();

        let opening_bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary {
                    code: Some("OPBD".to_string()),
                },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "100.00".to_string(),
            },
            cdt_dbt_ind: Some("CRDT".to_string()),
            date: None,
        };

        let closing_bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary {
                    code: Some("CLBD".to_string()),
                },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "200.00".to_string(),
            },
            cdt_dbt_ind: Some("CRDT".to_string()),
            date: None,
        };

        stmt.balances.push(opening_bal);
        stmt.balances.push(closing_bal);

        let (opening, closing) = extract_balances(&stmt);

        assert!(opening.is_some());
        assert!(closing.is_some());
    }

    #[test]
    fn extract_balances_ignores_unknown_balance_types() {
        let mut stmt = empty_statement();

        let other_bal = Camt053Balance {
            balance_type: Camt053BalanceType {
                code_or_proprietary: Camt053BalanceCodeOrProprietary {
                    code: Some("INFO".to_string()),
                },
            },
            amount: CamtAmtXml {
                currency: "EUR".to_string(),
                value: "999.99".to_string(),
            },
            cdt_dbt_ind: Some("CRDT".to_string()),
            date: None,
        };

        stmt.balances.push(other_bal);

        let (opening, closing) = extract_balances(&stmt);

        assert!(opening.is_none());
        assert!(closing.is_none());
    }

    // parse_camt_date_to_naive

    #[test]
    fn parse_camt_date_handles_plain_date() {
        let d = parse_camt_date_to_naive("2023-04-20").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2023, 4, 20).unwrap());
    }

    #[test]
    fn parse_camt_date_handles_datetime() {
        let d = parse_camt_date_to_naive("2023-04-20T23:59:59").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2023, 4, 20).unwrap());
    }

    #[test]
    fn parse_camt_date_fails_on_invalid_string() {
        let err = parse_camt_date_to_naive("not-a-date").unwrap_err();
        match err {
            ParseError::BadInput(msg) => {
                assert!(msg.contains("invalid CAMT date"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // detect_period

    #[test]
    fn detect_period_uses_explicit_period_if_present() {
        let mut stmt = empty_statement();
        stmt.period = Some(Camt053Period {
            from: Some("2023-01-01T00:00:00".to_string()),
            to: Some("2023-01-31T23:59:59".to_string()),
        });

        let (from, to) = detect_period(&stmt).unwrap();

        assert_eq!(from, NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2023, 1, 31).unwrap());
    }

    #[test]
    fn detect_period_falls_back_to_min_max_entry_dates() {
        let mut stmt = empty_statement();

        stmt.entries.push(Camt053Entry {
            booking_date: CamtDateXml {
                date: "2023-02-10".to_string(),
            },
            ..Default::default()
        });

        stmt.entries.push(Camt053Entry {
            booking_date: CamtDateXml {
                date: "2023-02-15".to_string(),
            },
            ..Default::default()
        });

        stmt.entries.push(Camt053Entry {
            booking_date: CamtDateXml {
                date: "2023-02-05".to_string(),
            },
            ..Default::default()
        });

        let (from, to) = detect_period(&stmt).unwrap();

        assert_eq!(from, NaiveDate::from_ymd_opt(2023, 2, 5).unwrap());
        assert_eq!(to, NaiveDate::from_ymd_opt(2023, 2, 15).unwrap());
    }

    #[test]
    fn detect_period_fails_if_no_period_and_no_entries() {
        let stmt = empty_statement();
        let err = detect_period(&stmt).unwrap_err();

        match err {
            ParseError::BadInput(msg) => {
                assert!(msg.contains("missing camt statement period"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    // counterparty_from_tx

    fn make_party(name: &str) -> CamtParty {
        CamtParty {
            name: Some(name.to_string()),
            postal_address: None,
            id: None,
        }
    }

    fn make_account(iban: &str) -> CamtAccount {
        CamtAccount {
            id: CamtAccountId {
                iban: Some(iban.to_string()),
            },
        }
    }

    #[test]
    fn counterparty_from_tx_prefers_ultimate_creditor_for_debit() {
        let parties = CamtRelatedParties {
            ultimate_creditor: Some(make_party("Ultimate Creditor")),
            creditor: Some(make_party("Normal Creditor")),
            creditor_account: Some(make_account("CRED_IBAN")),
            ..Default::default()
        };

        let tx = CamtTxDtls {
            related_parties: Some(parties),
            ..Default::default()
        };

        let (cp_id, cp_name) = counterparty_from_tx(&tx, Direction::Debit);

        assert_eq!(cp_id, Some("CRED_IBAN".to_string()));
        assert_eq!(cp_name, Some("Ultimate Creditor".to_string()));
    }

    #[test]
    fn counterparty_from_tx_prefers_ultimate_debtor_for_credit() {
        let parties = CamtRelatedParties {
            ultimate_debtor: Some(make_party("Ultimate Debtor")),
            debtor: Some(make_party("Normal Debtor")),
            debtor_account: Some(make_account("DEBT_IBAN")),
            ..Default::default()
        };

        let tx = CamtTxDtls {
            related_parties: Some(parties),
            ..Default::default()
        };

        let (cp_id, cp_name) = counterparty_from_tx(&tx, Direction::Credit);

        assert_eq!(cp_id, Some("DEBT_IBAN".to_string()));
        assert_eq!(cp_name, Some("Ultimate Debtor".to_string()));
    }

    #[test]
    fn counterparty_from_tx_uses_non_ultimate_if_ultimate_missing() {
        // Debit: должно взять creditor, если ultimate_creditor нет
        let parties = CamtRelatedParties {
            creditor: Some(make_party("Creditor Only")),
            creditor_account: Some(make_account("CRED_ONLY_IBAN")),
            ..Default::default()
        };

        let tx = CamtTxDtls {
            related_parties: Some(parties),
            ..Default::default()
        };

        let (cp_id, cp_name) = counterparty_from_tx(&tx, Direction::Debit);

        assert_eq!(cp_id, Some("CRED_ONLY_IBAN".to_string()));
        assert_eq!(cp_name, Some("Creditor Only".to_string()));
    }

    #[test]
    fn counterparty_from_tx_returns_none_if_no_related_parties() {
        let tx = CamtTxDtls {
            related_parties: None,
            ..Default::default()
        };

        let (cp_id, cp_name) = counterparty_from_tx(&tx, Direction::Credit);

        assert!(cp_id.is_none());
        assert!(cp_name.is_none());
    }

    // description_from_tx

    #[test]
    fn description_from_tx_joins_unstructured_with_newlines() {
        let rmt = CamtRemittanceInfo {
            unstructured: vec![
                "Line 1".to_string(),
                "Line 2".to_string(),
                "Line 3".to_string(),
            ],
            structured: vec![],
        };

        let tx = CamtTxDtls {
            rmt_inf: Some(rmt),
            ..Default::default()
        };

        let desc = description_from_tx(&tx);

        assert_eq!(desc, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn description_from_tx_is_empty_if_no_remittance() {
        let tx = CamtTxDtls {
            rmt_inf: None,
            ..Default::default()
        };

        let desc = description_from_tx(&tx);
        assert_eq!(desc, "");
    }

    #[test]
    fn description_from_tx_is_empty_if_unstructured_empty() {
        let rmt = CamtRemittanceInfo {
            unstructured: vec![],
            structured: vec![],
        };

        let tx = CamtTxDtls {
            rmt_inf: Some(rmt),
            ..Default::default()
        };

        let desc = description_from_tx(&tx);
        assert_eq!(desc, "");
    }
}
