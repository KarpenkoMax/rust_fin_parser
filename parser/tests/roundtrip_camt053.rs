use parser::{Camt053Data, Direction, Statement};
use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::PathBuf,
};

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("camt053")
        .join("camt053_example")
}

fn parse_camt053_to_statement() -> Statement {
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
fn camt053_roundtrip_via_statement_preserves_core_data() {
    // исходный Statement из фикстуры
    let original = parse_camt053_to_statement();

    assert!(
        !original.transactions.is_empty(),
        "fixture CAMT053 should contain at least one transaction"
    );

    // сериализуем Statement в CAMT053 XML
    let mut buf: Vec<u8> = Vec::new();
    original
        .write_camt053(&mut buf)
        .expect("failed to write Statement back to CAMT053");

    // снова парсим CAMT053 в Statement
    let cursor = Cursor::new(&buf);
    let camt2 = Camt053Data::parse(cursor).expect("failed to parse roundtripped CAMT053 XML");
    let roundtrip: Statement = camt2
        .try_into()
        .expect("failed to convert roundtripped Camt053Data into Statement");

    // Счёт и валюта
    assert_eq!(
        original.account_id, roundtrip.account_id,
        "account_id should be preserved after CAMT053 roundtrip"
    );
    assert_eq!(
        original.currency, roundtrip.currency,
        "currency should be preserved after CAMT053 roundtrip"
    );

    // Балансы
    assert_eq!(
        original.opening_balance, roundtrip.opening_balance,
        "opening balance should be preserved after CAMT053 roundtrip"
    );
    assert_eq!(
        original.closing_balance, roundtrip.closing_balance,
        "closing balance should be preserved after CAMT053 roundtrip"
    );

    // Период
    assert_eq!(
        original.period_from, roundtrip.period_from,
        "period_from should be preserved after CAMT053 roundtrip"
    );
    assert_eq!(
        original.period_until, roundtrip.period_until,
        "period_until should be preserved after CAMT053 roundtrip"
    );

    // Количество транзакций
    assert_eq!(
        original.transactions.len(),
        roundtrip.transactions.len(),
        "number of transactions should be preserved after CAMT053 roundtrip"
    );

    // сравниваем транзакции поштучно
    for (i, (orig_tx, rt_tx)) in original
        .transactions
        .iter()
        .zip(roundtrip.transactions.iter())
        .enumerate()
    {
        // даты
        assert_eq!(
            orig_tx.booking_date, rt_tx.booking_date,
            "booking_date mismatch at transaction #{i}"
        );
        assert_eq!(
            orig_tx.value_date, rt_tx.value_date,
            "value_date mismatch at transaction #{i}"
        );

        // сумма и направление
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

        // описание
        assert_eq!(
            orig_tx.description, rt_tx.description,
            "description mismatch at transaction #{i}"
        );

        // контрагент
        assert_eq!(
            orig_tx.counterparty, rt_tx.counterparty,
            "counterparty mismatch at transaction #{i}"
        );

        assert_eq!(
            orig_tx.counterparty_name, rt_tx.counterparty_name,
            "counterparty_name mismatch at transaction #{i}"
        );
    }
}
