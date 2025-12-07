use parser::{Mt940Data, Statement, Direction};
use std::{
    fs::File,
    io::BufReader,
    path::PathBuf,
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("mt940")
        .join("example.mt940")
}

fn parse_mt940_to_statement() -> Statement {
    let path = fixture_path();
    let file = File::open(&path)
        .unwrap_or_else(|e| panic!("failed to open MT940 fixture {path:?}: {e}"));
    let reader = BufReader::new(file);

    let data = Mt940Data::parse(reader).expect("failed to parse MT940 fixture");
    let stmt: Statement = data
        .try_into()
        .expect("failed to convert Mt940Data into Statement");

    stmt
}

#[test]
fn mt940_example_parses_into_non_empty_statement() {
    let stmt = parse_mt940_to_statement();

    // базовые sanity-чекы
    assert!(
        !stmt.transactions.is_empty(),
        "MT940 fixture should produce at least one transaction"
    );

    // из фикстуры:
    // :25:107048825
    assert_eq!(
        stmt.account_id, "107048825",
        "account_id from MT940 :25: should be parsed"
    );

    // :60M:C250218USD2732398848,02 d USD
    use parser::Currency;
    assert_eq!(
        stmt.currency,
        Currency::USD,
        "currency should be taken from opening balance :60M:"
    );

    // открывающий / закрывающий баланс из :60M: и :62M:
    assert!(
        stmt.opening_balance.is_some(),
        "opening_balance should be present from :60M:"
    );
    assert!(
        stmt.closing_balance.is_some(),
        "closing_balance should be present from :62M:"
    );

    // период - по датам балансов
    use chrono::NaiveDate;
    let expected_date = NaiveDate::from_ymd_opt(2025, 2, 18).unwrap();
    assert_eq!(
        stmt.period_from, expected_date,
        "period_from should be derived from opening balance date (250218)"
    );
    assert_eq!(
        stmt.period_until, expected_date,
        "period_until should be derived from closing balance date (250218)"
    );

    // в фикстуре 4 проводки :61:
    assert_eq!(
        stmt.transactions.len(),
        4,
        "MT940 example.mt940 is expected to contain 4 transactions"
    );

    // Проверим первую и последнюю транзакции по направлению и сумме
    let first = &stmt.transactions[0];
    assert_eq!(
        first.direction,
        Direction::Debit,
        "first transaction should be Debit (D in :61:)"
    );
    assert_eq!(
        first.amount, 1201,
        "first transaction amount (12,01) should be parsed as 1201 minor units"
    );

    let last = &stmt.transactions[3];
    assert_eq!(
        last.direction,
        Direction::Credit,
        "last transaction should be Credit (C in :61:)"
    );
    assert_eq!(
        last.amount, 1125,
        "last transaction amount (11,25) should be parsed as 1125 minor units"
    );
}
