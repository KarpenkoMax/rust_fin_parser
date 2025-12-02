use crate::model::{Balance, Direction};
use crate::error::ParseError;
use csv::{StringRecord};
use crate::utils::parse_amount;


pub(crate) fn parse_footer_balance(row: &StringRecord) -> Result<Balance, ParseError> {
    let debit  = row.get(7).map(str::trim).filter(|s| !s.is_empty());
    let credit = row.get(11).map(str::trim).filter(|s| !s.is_empty());

    let raw = debit
        .filter(|s| *s != "0,00" && *s != "0.00")
        .or_else(|| credit.filter(|s| *s != "0,00" && *s != "0.00"))
        .unwrap_or("0.00");

    let normalized = raw.replace(',', ".");
    let amount = parse_amount(&normalized)?;
    Ok(amount as Balance)
}

/// Возвращает:
/// - 1-ю непустую строку как номер счёта
/// - 3-ю непустую строку как имя контрагента
pub(crate) fn extract_account_and_name(block: &str) -> (Option<String>, Option<String>) {
    let lines: Vec<_> = block
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();

    let account = lines.get(0).map(|s| (*s).to_string());
    let name    = lines.get(2).map(|s| (*s).to_string());

    (account, name)
}

/// Определяет счёт и имя контрагента:
/// - если наш счёт в дебете - контрагент = (счёт, имя) из кредитового блока
/// - если наш счёт в кредите - контрагент = (счёт, имя) из дебетового блока
/// - иначе - (None, None)
pub(crate) fn extract_counterparty_account(
    debit_block: &str,
    credit_block: &str,
    our_account: &str,
) -> (Option<String>, Option<String>) {
    let (debit_acc,  debit_name)  = extract_account_and_name(debit_block);
    let (credit_acc, credit_name) = extract_account_and_name(credit_block);

    // наш счёт в дебете - к нам пришли деньги
    if let Some(acc) = debit_acc.as_deref() {
        if acc == our_account {
            return (credit_acc, credit_name);
        }
    }

    // наш счёт в кредите - от нас ушли деньги
    if let Some(acc) = credit_acc.as_deref() {
        if acc == our_account {
            return (debit_acc, debit_name);
        }
    }

    (None, None)
}

pub(crate) fn parse_amount_and_direction(
    debit: Option<&str>,
    credit: Option<&str>,
) -> Result<(u64, Direction), ParseError> {

    fn is_empty(val: Option<&str>) -> bool {
        if let Some(s) = val {
            s.trim().is_empty()
        } else {
            true
        }
    }

    match (debit, credit) {
        // дебет: значение есть и непустое, кредит пустой/отсутствует
        (Some(d), c) if !d.trim().is_empty() && is_empty(c) => {
            let amount = parse_amount(d)?;
            let direction = Direction::Debit;
            Ok((amount, direction))
        },
        // кредит: значение есть и непустое, дебет пустой/отсутствует
        (d, Some(c)) if !c.trim().is_empty() && is_empty(d) => {
            let amount = parse_amount(c)?;
            let direction = Direction::Credit;
            Ok((amount, direction))
        },
        _ => Err(ParseError::AmountSideConflict)
    }
}

pub(crate) fn is_footer_row(row: &StringRecord) -> bool {
    row.iter().any(|field| {
        let field = field.trim();
        field == "б/с"
            || field.starts_with("Количество операций")
            || field.starts_with("Входящий остаток")
            || field.starts_with("Исходящий остаток")
            || field.starts_with("Итого оборотов")
    })
}

/// Ищет индекс колонки, содержащей текст
/// 
/// Возвращает первый найденный, если не находит - возвращает ошибку
pub(crate) fn find_col(row: &StringRecord, needle: &str) -> Result<usize, ParseError> {
    row.iter()
        .position(|field| field.contains(needle))
        .ok_or_else(|| ParseError::Header(
            format!("column with header containing '{needle}' not found")
        ))
}

