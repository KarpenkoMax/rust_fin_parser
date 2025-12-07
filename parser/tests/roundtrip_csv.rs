use parser::{CsvData, Statement, Direction};
use std::{fs::File, io::{BufReader, Cursor}, path::PathBuf};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("csv")
        .join("example.csv")
}

fn parse_csv_to_statement() -> Statement {
    let path = fixture_path();
    let file = File::open(&path)
        .unwrap_or_else(|e| panic!("failed to open CSV fixture {path:?}: {e}"));
    let reader = BufReader::new(file);

    let csv_data = CsvData::parse(reader).expect("failed to parse CSV fixture");
    let stmt: Statement = csv_data
        .try_into()
        .expect("failed to convert CsvData into Statement");

    stmt
}

fn normalize_name(name: &Option<String>) -> Option<String> {
    match name.as_deref().map(str::trim) {
        None => None,
        Some("") => None,
        Some("-") => None,
        Some(other) => Some(other.to_string()),
    }
}

#[test]
fn csv_roundtrip_via_statement_preserves_core_data() {
    // исходный Statement из фикстуры
    let original = parse_csv_to_statement();

    assert!(
        !original.transactions.is_empty(),
        "fixture CSV should contain at least one transaction"
    );

    // сериализуем в CSV
    let mut buf: Vec<u8> = Vec::new();
    original
        .write_csv(&mut buf)
        .expect("failed to write Statement back to CSV");

    // снова парсим CSV в Statement
    let cursor = Cursor::new(&buf);
    let csv2 = CsvData::parse(cursor).expect("failed to parse roundtripped CSV");
    let roundtrip: Statement = csv2
        .try_into()
        .expect("failed to convert roundtripped CsvData into Statement");

    // сравниваем ключевые поля

    // Счёт и валюта должны совпасть
    assert_eq!(
        original.account_id, roundtrip.account_id,
        "account_id should be preserved after CSV roundtrip"
    );
    assert_eq!(
        original.currency, roundtrip.currency,
        "currency should be preserved after CSV roundtrip"
    );

    // Открывающий/закрывающий балансы - если были заданы, должны совпасть
    assert_eq!(
        original.opening_balance, roundtrip.opening_balance,
        "opening balance should be preserved after CSV roundtrip"
    );
    assert_eq!(
        original.closing_balance, roundtrip.closing_balance,
        "closing balance should be preserved after CSV roundtrip"
    );

    // период
    assert_eq!(
        original.period_from, roundtrip.period_from,
        "period_from should be preserved after CSV roundtrip"
    );
    assert_eq!(
        original.period_until, roundtrip.period_until,
        "period_until should be preserved after CSV roundtrip"
    );

    // Количество транзакций
    assert_eq!(
        original.transactions.len(),
        roundtrip.transactions.len(),
        "number of transactions should be preserved after CSV roundtrip"
    );

    // Сравниваем поштучно базовые поля транзакций
    for (i, (orig_tx, rt_tx)) in original
        .transactions
        .iter()
        .zip(roundtrip.transactions.iter())
        .enumerate()
    {
        assert_eq!(
            orig_tx.booking_date, rt_tx.booking_date,
            "booking_date mismatch at transaction #{i}"
        );

        assert_eq!(
            orig_tx.amount, rt_tx.amount,
            "amount mismatch at transaction #{i}"
        );

        assert!(
            matches!(orig_tx.direction, Direction::Debit | Direction::Credit),
            "original tx #{i} has unexpected direction"
        );
        assert_eq!(
            orig_tx.direction, rt_tx.direction,
            "direction mismatch at transaction #{i}"
        );

        assert_eq!(
            orig_tx.description, rt_tx.description,
            "description mismatch at transaction #{i}"
        );

        assert_eq!(
            orig_tx.counterparty, rt_tx.counterparty,
            "counterparty mismatch at transaction #{i}"
        );

        let norm_orig_cp_name = normalize_name(&orig_tx.counterparty_name);
        let norm_rt_cp_name = normalize_name(&rt_tx.counterparty_name);

        assert_eq!(
            norm_orig_cp_name,
            norm_rt_cp_name,
            "counterparty_name mismatch at transaction #{i}"
        );
    }
}
