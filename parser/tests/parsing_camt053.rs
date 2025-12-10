use chrono::NaiveDate;
use parser::{Camt053Data, Direction, Statement};
use std::{fs::File, io::BufReader, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("camt053")
        .join("camt053_example")
}

fn parse_camt053_fixture() -> Statement {
    let path = fixture_path();
    let file = File::open(&path)
        .unwrap_or_else(|e| panic!("failed to open CAMT053 fixture {path:?}: {e}"));
    let reader = BufReader::new(file);

    let camt_data = Camt053Data::parse(reader).expect("failed to parse CAMT053 fixture");
    let stmt: Statement = camt_data
        .try_into()
        .expect("failed to convert Camt053Data into Statement");

    stmt
}

#[test]
fn camt053_danske_example_parses_and_has_expected_metadata() {
    let stmt = parse_camt053_fixture();

    // IBAN из <Acct><Id><IBAN>
    assert_eq!(stmt.account_id, "DK8030000001234567");

    // Имя счёта
    assert_eq!(stmt.account_name.as_deref(), Some("Danske Corporate"));

    // 6 <Ntry> => 6 транзакций
    assert_eq!(stmt.transactions.len(), 6);

    // Балансы должны быть
    assert!(
        stmt.opening_balance.is_some(),
        "opening balance should be present"
    );
    assert!(
        stmt.closing_balance.is_some(),
        "closing balance should be present"
    );

    // Период должен быть хотя бы корректным по порядку
    assert!(
        stmt.period_from <= stmt.period_until,
        "period_from must be <= period_until (got {} > {})",
        stmt.period_from,
        stmt.period_until
    );
}

#[test]
fn camt053_danske_example_first_and_last_entries_look_ok() {
    let stmt = parse_camt053_fixture();

    let first = &stmt.transactions[0];
    assert!(
        matches!(first.direction, Direction::Credit),
        "first entry should be credit"
    );
    assert_eq!(
        first.booking_date,
        NaiveDate::from_ymd_opt(2023, 4, 20).unwrap()
    );

    let last = stmt.transactions.last().expect("no last transaction");
    assert!(
        matches!(last.direction, Direction::Debit),
        "last entry should be debit"
    );
    assert_eq!(
        last.booking_date,
        NaiveDate::from_ymd_opt(2023, 5, 9).unwrap()
    );
}
