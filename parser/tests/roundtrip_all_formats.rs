use parser::{Camt053Data, CsvData, Mt940Data, Statement, Direction};
use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::PathBuf,
};

fn camt_fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("camt053")
        .join("camt053_example")
}

fn parse_camt_to_statement() -> Statement {
    let path = camt_fixture_path();
    let file = File::open(&path)
        .unwrap_or_else(|e| panic!("failed to open CAMT053 fixture {path:?}: {e}"));
    let reader = BufReader::new(file);

    let camt = Camt053Data::parse(reader).expect("failed to parse CAMT053 fixture");
    camt.try_into()
        .expect("failed to convert Camt053Data into Statement")
}

#[test]
fn camt_to_csv_to_mt940_roundtrip_preserves_core_data() {
    // Исходный Statement из CAMT
    let original = parse_camt_to_statement();

    assert!(
        !original.transactions.is_empty(),
        "CAMT053 fixture should contain at least one transaction"
    );

    // CAMT Statement в CSV
    let mut csv_buf: Vec<u8> = Vec::new();
    original
        .write_csv(&mut csv_buf)
        .expect("failed to write Statement as CSV");

    // CSV в Statement
    let csv_cursor = Cursor::new(&csv_buf);
    let csv_data =
        CsvData::parse(csv_cursor).expect("failed to parse intermediate CSV");
    let csv_stmt: Statement = csv_data
        .try_into()
        .expect("failed to convert intermediate CsvData into Statement");

    assert!(
        !csv_stmt.transactions.is_empty(),
        "Statement after CSV step should not be empty"
    );

    // Statement (после CSV) в MT940
    let mut mt940_buf: Vec<u8> = Vec::new();
    csv_stmt
        .write_mt940(&mut mt940_buf)
        .expect("failed to write Statement as MT940");

    // MT940 в финальный Statement
    let mt_cursor = Cursor::new(&mt940_buf);
    let mt_data =
        Mt940Data::parse(mt_cursor).expect("failed to parse intermediate MT940");
    let final_stmt: Statement = mt_data
        .try_into()
        .expect("failed to convert intermediate Mt940Data into Statement");

    assert!(
        !final_stmt.transactions.is_empty(),
        "final Statement after MT940 should not be empty"
    );

    // Сравнение core-данных original vs final_stmt

    // Счёт и валюта
    assert_eq!(
        original.account_id, final_stmt.account_id,
        "account_id should be preserved after CAMTвCSVвMT940 roundtrip"
    );
    assert_eq!(
        original.currency, final_stmt.currency,
        "currency should be preserved after CAMTвCSVвMT940 roundtrip"
    );

    // Балансы
    assert_eq!(
        original.opening_balance, final_stmt.opening_balance,
        "opening_balance should be preserved after CAMTвCSVвMT940 roundtrip"
    );
    assert_eq!(
        original.closing_balance, final_stmt.closing_balance,
        "closing_balance should be preserved after CAMTвCSVвMT940 roundtrip"
    );

    // Период
    assert_eq!(
        original.period_from, final_stmt.period_from,
        "period_from should be preserved after CAMTвCSVвMT940 roundtrip"
    );
    assert_eq!(
        original.period_until, final_stmt.period_until,
        "period_until should be preserved after CAMTвCSVвMT940 roundtrip"
    );

    // Количество транзакций
    assert_eq!(
        original.transactions.len(),
        final_stmt.transactions.len(),
        "number of transactions should be preserved after CAMTвCSVвMT940 roundtrip"
    );

    // Сравнение транзакций поштучно
    for (i, (orig_tx, final_tx)) in original
        .transactions
        .iter()
        .zip(final_stmt.transactions.iter())
        .enumerate()
    {
        // даты
        assert_eq!(
            orig_tx.booking_date, final_tx.booking_date,
            "booking_date mismatch at transaction #{i}"
        );
        assert_eq!(
            orig_tx.value_date, final_tx.value_date,
            "value_date mismatch at transaction #{i}"
        );

        // сумма и направление
        assert_eq!(
            orig_tx.amount, final_tx.amount,
            "amount mismatch at transaction #{i}"
        );
        assert!(
            matches!(orig_tx.direction, Direction::Debit | Direction::Credit),
            "original tx #{i} has unexpected direction"
        );
        assert_eq!(
            orig_tx.direction, final_tx.direction,
            "direction mismatch at transaction #{i}"
        );

    }
}
