use parser::{CsvData, Statement};
use std::{fs::File, io::BufReader, path::PathBuf};

fn fixture_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(rel)
}

fn parse_csv_fixture() -> Statement {
    let path = fixture_path("csv/example.csv");
    let file =
        File::open(&path).unwrap_or_else(|e| panic!("failed to open CSV fixture {path:?}: {e}"));
    let reader = BufReader::new(file);

    let csv_data = CsvData::parse(reader).expect("failed to parse CSV fixture");
    let stmt: Statement = csv_data
        .try_into()
        .expect("failed to convert CsvData into Statement");

    stmt
}

#[test]
fn csv_example_parses_into_non_empty_statement() {
    let stmt = parse_csv_fixture();

    // есть хотя бы одна операция
    // opening / closing balance присутствуют
    assert!(
        !stmt.transactions.is_empty(),
        "statement should contain at least one transaction"
    );

    assert!(
        stmt.opening_balance.is_some(),
        "opening balance should be present"
    );
    assert!(
        stmt.closing_balance.is_some(),
        "closing balance should be present"
    );

    // период должен быть корректным
    assert!(
        stmt.period_from <= stmt.period_until,
        "period_from must be <= period_until (got {} > {})",
        stmt.period_from,
        stmt.period_until
    );
}

#[test]
fn csv_example_transactions_within_statement_period() {
    let stmt = parse_csv_fixture();

    for tx in &stmt.transactions {
        assert!(
            tx.booking_date >= stmt.period_from && tx.booking_date <= stmt.period_until,
            "transaction date {} must be within [{}, {}]",
            tx.booking_date,
            stmt.period_from,
            stmt.period_until
        );
    }
}
