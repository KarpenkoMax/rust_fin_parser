use parser::{Direction, Mt940Data, Statement};
use std::{
    fs::File,
    io::{BufReader, Cursor},
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
    let file =
        File::open(&path).unwrap_or_else(|e| panic!("failed to open MT940 fixture {path:?}: {e}"));
    let reader = BufReader::new(file);

    let data = Mt940Data::parse(reader).expect("failed to parse MT940 fixture");
    let stmt: Statement = data
        .try_into()
        .expect("failed to convert Mt940Data into Statement");

    stmt
}

#[test]
fn mt940_roundtrip_via_statement_preserves_core_data() {
    // исходный Statement из MT940-фикстуры
    let original = parse_mt940_to_statement();

    assert!(
        !original.transactions.is_empty(),
        "fixture MT940 should contain at least one transaction"
    );

    // сериализуем Statement обратно в MT940
    let mut buf: Vec<u8> = Vec::new();
    original
        .write_mt940(&mut buf)
        .expect("failed to write Statement back to MT940");

    // снова парсим MT940 в Statement
    let cursor = Cursor::new(&buf);
    let data2 = Mt940Data::parse(cursor).expect("failed to parse roundtripped MT940 data");
    let roundtrip: Statement = data2
        .try_into()
        .expect("failed to convert roundtripped Mt940Data into Statement");

    // сравниваем Statement по основным полям

    // Счёт и валюта
    assert_eq!(
        original.account_id, roundtrip.account_id,
        "account_id should be preserved after MT940 roundtrip"
    );
    assert_eq!(
        original.currency, roundtrip.currency,
        "currency should be preserved after MT940 roundtrip"
    );

    // Балансы
    assert_eq!(
        original.opening_balance, roundtrip.opening_balance,
        "opening balance should be preserved after MT940 roundtrip"
    );
    assert_eq!(
        original.closing_balance, roundtrip.closing_balance,
        "closing balance should be preserved after MT940 roundtrip"
    );

    // Период
    assert_eq!(
        original.period_from, roundtrip.period_from,
        "period_from should be preserved after MT940 roundtrip"
    );
    assert_eq!(
        original.period_until, roundtrip.period_until,
        "period_until should be preserved after MT940 roundtrip"
    );

    // Количество транзакций
    assert_eq!(
        original.transactions.len(),
        roundtrip.transactions.len(),
        "number of transactions should be preserved after MT940 roundtrip"
    );

    // 5) сравниваем транзакции поштучно
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
        assert!(
            rt_tx
                .description
                .contains(orig_tx.description.split(" | ").next().unwrap_or("")),
            "roundtrip description should contain original prefix at tx #{i}"
        );

        // counterparty: следим, чтобы не потерять
        match (&orig_tx.counterparty, &rt_tx.counterparty) {
            (Some(o), Some(r)) => assert_eq!(o, r, "counterparty mismatch at transaction #{i}"),
            (Some(o), None) => {
                panic!("lost counterparty after MT940 roundtrip at transaction #{i}: was {o:?}")
            }
            // orig None - ок, можем получить None или Some(...) после допущений при парсинге
            (None, _) => {}
        }

        // counterparty_name: следим, чтобы не потерять
        match (&orig_tx.counterparty_name, &rt_tx.counterparty_name) {
            (Some(o), Some(r)) => {
                assert_eq!(o, r, "counterparty_name mismatch at transaction #{i}")
            }
            (Some(o), None) => panic!(
                "lost counterparty_name after MT940 roundtrip at transaction #{i}: was {o:?}"
            ),
            (None, _) => {}
        }
    }
}
