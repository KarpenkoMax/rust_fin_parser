mod utils;
use crate::error::ParseError;
use crate::model::{Balance, Currency, Direction, Statement, Transaction};
use crate::utils::{parse_amount, parse_currency};
use chrono::NaiveDate;
use std::io::{BufReader, Read};
use utils::*;

#[derive(Debug, Clone)]
pub struct Mt940Message {
    /// :20: Transaction Reference Number (может быть пустым у некоторых банков)
    pub transaction_reference: Option<String>,

    /// :25: Account Identification (номер счёта/IBAN как есть)
    pub account_id: String,

    /// :28C: Statement Number/Sequence, сырой текст, например "49/2" или "00001/001"
    pub statement_number: Option<String>,

    /// :60F: / :60M: Opening Balance
    pub opening_balance: Mt940Balance,

    /// Список всех проводок (:61: + связанные текстовые блоки, включая :86: и голые строки)
    pub entries: Vec<Mt940Entry>,

    /// :62F: Closing Balance (может отсутствовать в кривых файлах)
    pub closing_balance: Option<Mt940Balance>,

    /// :64: Closing Available Balance (доступный баланс), опционально
    pub closing_available_balance: Option<Mt940Balance>,
}

fn parse_balance(value: &str) -> Result<Mt940Balance, ParseError> {
    let value = value.trim();

    // минимум: 1 (C/D) + 6 (дата) + 3 (валюта) + 1 (хотя бы один символ суммы) = 11
    if value.len() < 11 {
        return Err(ParseError::BadInput(format!(
            "balance value too short: '{value}'"
        )));
    }

    // 1 символ C/D
    let mut chars = value.chars();
    let dc_mark = chars
        .next()
        .ok_or_else(|| ParseError::BadInput("empty balance value".into()))?;

    // value уже без первого символа
    let rest = &value[1..];

    // YYMMDD
    if rest.len() < 9 {
        return Err(ParseError::BadInput(format!(
            "balance value too short for date+currency: '{value}'"
        )));
    }

    let date = &rest[0..6];
    let currency = &rest[6..9];
    let amount = &rest[9..];

    Ok(Mt940Balance {
        dc_mark,
        date: date.to_string(),
        currency: currency.to_string(),
        amount: amount.trim().to_string(),
    })
}

impl Mt940Message {
    pub(crate) fn from_string_lines(lines: &[String]) -> Result<Self, ParseError> {
        let mut tx_ref: Option<String> = None; // :20:
        let mut account_id: Option<String> = None; // :25:
        let mut statement_number: Option<String> = None; // :28C:

        let mut opening_balance: Option<Mt940Balance> = None; // :60F: / :60M:
        let mut closing_balance: Option<Mt940Balance> = None; // :62F:
        let mut closing_available_balance: Option<Mt940Balance> = None; // :64:

        let mut entries: Vec<Mt940Entry> = Vec::new();
        let mut current_entry: Option<Mt940Entry> = None;

        for raw_line in lines {
            let line = raw_line.trim_end_matches('\r');
            let line_trimmed = line.trim_start();

            if line_trimmed.starts_with(':') {
                let (tag, value) = split_tag_line(line_trimmed)?;

                match tag {
                    "20" => {
                        tx_ref = Some(value.to_string());
                    }
                    "25" => {
                        account_id = Some(value.to_string());
                    }
                    "28C" => {
                        statement_number = Some(value.to_string());
                    }
                    "60F" | "60M" => {
                        let bal = parse_balance(value)?;
                        // первый 60* считаем opening_balance
                        if opening_balance.is_none() {
                            opening_balance = Some(bal);
                        } else {
                            eprintln!("multiple :60: opening balances, keeping the first one");
                        }
                    }
                    "62F" | "62M" => {
                        let bal = parse_balance(value)?;
                        closing_balance = Some(bal);
                    }
                    "64" => {
                        let bal = parse_balance(value)?;
                        closing_available_balance = Some(bal);
                    }
                    "61" => {
                        // закрываем предыдущую проводку
                        if let Some(entry) = current_entry.take() {
                            entries.push(entry);
                        }
                        current_entry =
                            Some(Mt940Entry::from_61_line(value, line_trimmed.to_string())?);
                    }
                    "86" => {
                        if let Some(entry) = current_entry.as_mut() {
                            entry.push_info_line(value);
                        }
                    }
                    other => {
                        eprintln!("skipped unknown tag {other}: {value}");
                    }
                }
            } else {
                // строка без ':', продолжение описания
                if let Some(entry) = current_entry.as_mut() {
                    entry.push_info_line(line_trimmed);
                }
            }
        }

        // не забываем последнюю проводку
        if let Some(entry) = current_entry.take() {
            entries.push(entry);
        }

        // проверяем обязательные поля
        let account_id = account_id
            .ok_or_else(|| ParseError::BadInput("MT940: missing :25: account id".into()))?;
        let opening_balance = opening_balance.ok_or_else(|| {
            ParseError::BadInput("MT940: missing opening balance :60F:/:60M:".into())
        })?;

        Ok(Mt940Message {
            transaction_reference: tx_ref,
            account_id,
            statement_number,
            opening_balance,
            entries,
            closing_balance,
            closing_available_balance,
        })
    }
}

impl TryFrom<Mt940Message> for Statement {
    type Error = ParseError;

    fn try_from(message: Mt940Message) -> Result<Self, Self::Error> {
        let Mt940Message {
            transaction_reference: _,
            account_id,
            statement_number: _,
            opening_balance: opening_mt,
            entries,
            closing_balance: closing_mt,
            closing_available_balance: _,
        } = message;

        // в MT940 обычно нет имени счёта
        let account_name: Option<String> = None;

        let currency: Currency = parse_currency(&opening_mt.currency);

        // открывающий баланс: строка суммы + знак C/D
        let opening_raw = parse_amount(&opening_mt.amount)? as i128;
        let opening_balance: Option<Balance> = Some(match opening_mt.dc_mark {
            'C' => opening_raw,
            'D' => -opening_raw,
            other => {
                return Err(ParseError::InvalidAmount(format!(
                    "unknown opening balance direction: {other}"
                )));
            }
        });

        let closing_balance: Option<Balance> = if let Some(cb) = &closing_mt {
            let raw = parse_amount(&cb.amount)? as i128;
            let signed = match cb.dc_mark {
                'C' => raw,
                'D' => -raw,
                other => {
                    return Err(ParseError::InvalidAmount(format!(
                        "unknown closing balance direction: {other}"
                    )));
                }
            };
            Some(signed)
        } else {
            None
        };

        let period_from: NaiveDate = parse_mt940_yy_mm_dd(&opening_mt.date)?;

        // конвертируем все Mt940Entry -> Transaction
        let mut transactions: Vec<Transaction> = Vec::with_capacity(entries.len());
        for entry in &entries {
            let tx = Transaction::try_from(entry)?;
            transactions.push(tx);
        }

        let period_until: NaiveDate = if let Some(cb) = &closing_mt {
            parse_mt940_yy_mm_dd(&cb.date)?
        } else {
            transactions
                .iter()
                .map(|tx| tx.booking_date)
                .max()
                .unwrap_or(period_from)
        };

        Ok(Statement::new(
            account_id,
            account_name,
            currency,
            opening_balance,
            closing_balance,
            transactions,
            period_from,
            period_until,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct Mt940Balance {
    /// 'C' или 'D' из тега (Credit/Debit mark)
    pub dc_mark: char,

    /// Дата в формате YYMMDD, ровно как в файле, напр. "250218"
    pub date: String,

    /// Код валюты, как есть: "EUR", "USD", "CHF", ...
    pub currency: String,

    /// Сумма, как в файле: "2732398848,02", "1000, 00"
    pub amount: String,
}

#[derive(Debug, Clone, Default)]
pub struct Mt940EntryInfo {
    /// Все строки текста, относящиеся к этой проводке,
    /// уже без начальных ":86:" / ": 86:" / прочих служебных префиксов.
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Mt940Entry {
    /// Оригинальная строка :61:
    pub raw_61: String,

    /// value date в формате YYMMDD
    pub value_date: String,

    /// entry date (дата проводки) в формате MMDD или DD
    pub entry_date: Option<String>,

    /// 'C' или 'D' - признак кредит/дебет в :61:
    pub dc_mark: char,

    /// Дополнительный символ-флаг после C/D (напр. 'R' в "DR"), если есть
    pub funds_code: Option<char>,

    /// Сумма из :61:, как строка, напр. "12,01", "65,00"
    pub amount: String,

    /// Тип операции (4 буквы) из :61:, напр. "NTRF", "NOVB", "OONM", если есть
    pub transaction_type: Option<String>,

    /// customer reference - часть после суммы и типа операции, ДО `//`, если есть
    pub customer_reference: Option<String>,

    /// bank reference - часть ПОСЛЕ `//`, если есть
    pub bank_reference: Option<String>,

    /// Хвост строки :61:, если после референсов идёт ещё что-то, что ты не хочешь терять
    pub extra_details: Option<String>,

    /// Всё текстовое описание
    /// (из :86: и строк между :61: и следующими тегами)
    pub info: Mt940EntryInfo,
}

fn build_description(entry: &Mt940Entry) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(tt) = &entry.transaction_type {
        parts.push(tt.clone());
    }

    if let Some(cust) = &entry.customer_reference {
        parts.push(cust.clone());
    }

    if let Some(bank) = &entry.bank_reference {
        parts.push(format!("//{bank}"));
    }

    if let Some(extra) = &entry.extra_details {
        parts.push(extra.clone());
    }

    if !entry.info.lines.is_empty() {
        parts.push(entry.info.lines.join(" "));
    }

    if parts.is_empty() {
        entry.raw_61.clone()
    } else {
        parts.join(" | ")
    }
}

/// Поиск (counterparty, counterparty_name) в Mt940Entry
pub fn extract_counterparty_from_mt940(entry: &Mt940Entry) -> (Option<String>, Option<String>) {
    // Сначала пробуем текст из :86:
    if let Some((iban, name)) = find_iban_and_name_in_lines(&entry.info.lines) {
        return (Some(iban), name);
    }

    // Пробуем customer_reference
    if let Some(ref cref) = entry.customer_reference
        && let Some((iban, name)) = find_iban_and_name_in_line(cref)
    {
        return (Some(iban), name);
    }

    // Пробуем bank_reference
    if let Some(ref bref) = entry.bank_reference
        && let Some((iban, name)) = find_iban_and_name_in_line(bref)
    {
        return (Some(iban), name);
    }

    (None, None)
}

impl TryFrom<&Mt940Entry> for Transaction {
    type Error = ParseError;

    fn try_from(entry: &Mt940Entry) -> Result<Self, Self::Error> {
        let direction = match entry.dc_mark {
            'D' => Direction::Debit,
            'C' => Direction::Credit,
            other => {
                return Err(ParseError::InvalidAmount(format!(
                    "unknown direction: {other}"
                )));
            }
        };

        let amount = parse_amount(&entry.amount)?;

        let value_date = parse_mt940_yy_mm_dd(&entry.value_date)?;
        let booking_date = derive_booking_date(value_date, entry.entry_date.as_deref())?;

        let description = build_description(entry);
        let (counterparty, counterparty_name) = extract_counterparty_from_mt940(entry);

        Ok(Transaction {
            booking_date,
            value_date: Some(value_date),
            amount,
            direction,
            description,
            counterparty,
            counterparty_name,
        })
    }
}

impl Mt940Entry {
    pub fn push_info_line(&mut self, line: &str) {
        self.info.lines.push(line.trim().to_string());
    }

    pub fn from_61_line(value: &str, raw_61: String) -> Result<Self, ParseError> {
        let value = value.trim();
        let bytes = value.as_bytes();
        let len = bytes.len();

        if len < 8 {
            return Err(ParseError::BadInput(format!(
                "statement line too short: '{value}'"
            )));
        }

        // value date (YYMMDD)
        let value_date = &value[0..6];
        let mut idx = 6;

        // entry date (4 digits)
        let mut entry_date = None;
        if len >= idx + 4 && value[idx..idx + 4].chars().all(|c| c.is_ascii_digit()) {
            entry_date = Some(value[idx..idx + 4].to_string());
            idx += 4;
        }

        let (dc_mark, funds_code, amount, rest_after_amount) =
            parse_dc_and_amount(&value[idx..], value)?;

        let mut rest = rest_after_amount;

        let mut transaction_type = None;
        let mut customer_reference = None;
        let mut bank_reference = None;
        let mut extra_details = None;

        // transaction_type: 4 буквы подряд
        if rest.len() >= 4 && rest[..4].chars().all(|c| c.is_ascii_alphabetic()) {
            transaction_type = Some(rest[..4].to_string());
            rest = rest[4..].trim_start();
        }

        if let Some(pos) = rest.find("//") {
            // есть customer_ref и bank_ref
            let (cust, after_cust) = rest.split_at(pos);
            customer_reference = Some(cust.trim().to_string());

            let after = &after_cust[2..]; // без //
            if let Some(space_pos) = after.find(' ') {
                let (bank, extra) = after.split_at(space_pos);
                bank_reference = Some(bank.trim().to_string());
                let extra = extra.trim();
                if !extra.is_empty() {
                    extra_details = Some(extra.to_string());
                }
            } else {
                let bank = after.trim();
                if !bank.is_empty() {
                    bank_reference = Some(bank.to_string());
                }
            }
        } else if !rest.is_empty() {
            // только customer_reference без // (напр. "NOVBNL47INGB9999999999")
            customer_reference = Some(rest.trim().to_string());
        }

        Ok(Mt940Entry {
            raw_61,
            value_date: value_date.to_string(),
            entry_date,
            dc_mark,
            funds_code,
            amount,
            transaction_type,
            customer_reference,
            bank_reference,
            extra_details,
            info: Mt940EntryInfo { lines: Vec::new() },
        })
    }
}

/// Структура с сырыми данными формата mt940.
///
/// Для парсинга используйте [`Mt940Data::parse`].
///
/// Пример:
/// ```rust,no_run
/// use std::io::Cursor;
/// use parser::Mt940Data;
/// # use parser::ParseError;
/// # fn main() -> Result<(), ParseError> {
/// let reader = Cursor::new(b":20:ABC\n:25:ACCOUNT\n");
/// let data = Mt940Data::parse(reader)?;
/// #     Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Mt940Data {
    /// Пока один Statement
    pub message: Mt940Message,
}

impl Mt940Data {
    /// Парсит при помощи переданного reader данные  в [`Mt940Data`]
    ///
    /// При ошибке возвращает [`ParseError`]
    pub fn parse<R: Read>(reader: R) -> Result<Self, ParseError> {
        use std::io::BufRead;

        let buf_reader = BufReader::new(reader);
        let mut messages: Vec<Mt940Message> = Vec::new();
        let mut message_lines: Vec<String> = Vec::new();

        #[derive(Copy, Clone, Debug)]
        enum BlockKind {
            Curly, // {4: ... -}
            Paren, // (4: ... -)
        }

        let mut block_kind: Option<BlockKind> = None;
        let mut in_text_block = false;

        for line_result in buf_reader.lines() {
            let line = line_result?;
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            // ещё не внутри блока {4:/ (4:
            if !in_text_block {
                match block_kind {
                    Some(BlockKind::Curly) => {
                        if let Some(pos) = line.find("{4:") {
                            in_text_block = true;
                            let after = &line[pos + 3..];
                            if !after.trim().is_empty() {
                                message_lines.push(after.to_string());
                            }
                        }
                    }
                    Some(BlockKind::Paren) => {
                        if let Some(pos) = line.find("(4:") {
                            in_text_block = true;
                            let after = &line[pos + 3..];
                            if !after.trim().is_empty() {
                                message_lines.push(after.to_string());
                            }
                        }
                    }
                    None => {
                        // первый раз определяем тип блока: что встретится раньше
                        let pos_curly = line.find("{4:");
                        let pos_paren = line.find("(4:");

                        let (kind, pos) = match (pos_curly, pos_paren) {
                            (Some(pc), Some(pp)) => {
                                if pc <= pp {
                                    (BlockKind::Curly, pc)
                                } else {
                                    (BlockKind::Paren, pp)
                                }
                            }
                            (Some(pc), None) => (BlockKind::Curly, pc),
                            (None, Some(pp)) => (BlockKind::Paren, pp),
                            (None, None) => {
                                // в этой строке начала блока нет
                                continue;
                            }
                        };

                        block_kind = Some(kind);
                        in_text_block = true;

                        let after = &line[pos + 3..];
                        if !after.trim().is_empty() {
                            message_lines.push(after.to_string());
                        }
                    }
                }

                continue;
            }

            // внутри блока

            let kind = block_kind.expect("in_text_block set but block_kind is None");

            // закрывающие маркеры зависят от типа блока
            let close_markers: &[&str] = match kind {
                BlockKind::Curly => &["-}", "}"],
                BlockKind::Paren => &["-)", ")"],
            };

            if close_markers.iter().any(|p| trimmed.starts_with(p)) {
                // закончили один message
                let msg = Mt940Message::from_string_lines(&message_lines)?;
                messages.push(msg);

                message_lines.clear();
                in_text_block = false;
                continue;
            }

            // обычная строка тела message
            message_lines.push(line);
        }

        // файл закончился, но блок не закрыт
        if in_text_block && !message_lines.is_empty() {
            let msg = Mt940Message::from_string_lines(&message_lines)?;
            messages.push(msg);
        }

        if messages.is_empty() {
            return Err(ParseError::BadInput("0 mt940 messages detected".into()));
        }

        let mut messages_iter = messages.into_iter();
        let final_msg = messages_iter
            .next()
            .ok_or_else(|| ParseError::BadInput("0 mt940 messages detected".into()))?;

        if messages_iter.next().is_some() {
            eprintln!("more than one statement provided to mt940 parser. only reading first");
        }

        Ok(Mt940Data { message: final_msg })
    }
}

impl TryFrom<Mt940Data> for Statement {
    type Error = ParseError;

    fn try_from(data: Mt940Data) -> Result<Self, Self::Error> {
        Statement::try_from(data.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Currency, Direction};
    use chrono::NaiveDate;

    // parse_balance

    #[test]
    fn parse_balance_parses_valid_credit_balance() {
        // C + YYMMDD + CCY + amount
        let bal = parse_balance("C230101EUR123,45").unwrap();

        assert_eq!(bal.dc_mark, 'C');
        assert_eq!(bal.date, "230101");
        assert_eq!(bal.currency, "EUR");
        assert_eq!(bal.amount, "123,45");
    }

    #[test]
    fn parse_balance_parses_valid_debit_balance() {
        let bal = parse_balance("D250218USD1000,00").unwrap();

        assert_eq!(bal.dc_mark, 'D');
        assert_eq!(bal.date, "250218");
        assert_eq!(bal.currency, "USD");
        assert_eq!(bal.amount, "1000,00");
    }

    #[test]
    fn parse_balance_errors_on_too_short_value() {
        let err = parse_balance("C2301").unwrap_err();
        match err {
            ParseError::BadInput(msg) => {
                assert!(msg.contains("too short"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadInput, got {other:?}"),
        }
    }

    // Mt940Entry::from_61_line

    #[test]
    fn from_61_line_parses_minimal_line_with_entry_date() {
        // value_date=230101, entry_date=0102, C, amount=100,00
        let raw = ":61:2301010102C100,00";
        let entry = Mt940Entry::from_61_line("2301010102C100,00", raw.to_string()).unwrap();

        assert_eq!(entry.raw_61, raw);
        assert_eq!(entry.value_date, "230101");
        assert_eq!(entry.entry_date.as_deref(), Some("0102"));
        assert_eq!(entry.dc_mark, 'C');
        assert_eq!(entry.funds_code, None);
        assert_eq!(entry.amount, "100,00");
        assert!(entry.transaction_type.is_none());
        assert!(entry.customer_reference.is_none());
        assert!(entry.bank_reference.is_none());
        assert!(entry.extra_details.is_none());
    }

    #[test]
    fn from_61_line_parses_line_with_type_and_references_and_extra() {
        // 230101 value, 0102 entry, D, amount, NTRF type, custRef, bankRef, extra text
        let value = "2301010102D250,00NTRFREF123//BANKREF some extra text";
        let raw = format!(":61:{value}");

        let entry = Mt940Entry::from_61_line(value, raw.clone()).unwrap();

        assert_eq!(entry.raw_61, raw);
        assert_eq!(entry.value_date, "230101");
        assert_eq!(entry.entry_date.as_deref(), Some("0102"));
        assert_eq!(entry.dc_mark, 'D');
        assert_eq!(entry.amount, "250,00");
        assert_eq!(entry.transaction_type.as_deref(), Some("NTRF"));
        assert_eq!(entry.customer_reference.as_deref(), Some("REF123"));
        assert_eq!(entry.bank_reference.as_deref(), Some("BANKREF"));
        assert_eq!(entry.extra_details.as_deref(), Some("some extra text"));
    }

    #[test]
    fn from_61_line_errors_when_no_amount() {
        // value_date=230101, dc_mark=C, дальше только буквы
        let value = "230101CXXXX";
        let raw = format!(":61:{value}");

        let err = Mt940Entry::from_61_line(value, raw).unwrap_err();
        match err {
            ParseError::BadInput(msg) => {
                assert!(
                    msg.contains("no amount found in :61"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected BadInput, got {other:?}"),
        }
    }

    // build_description

    #[test]
    fn build_description_joins_all_parts() {
        let mut entry = Mt940Entry {
            raw_61: ":61:2301010102C100,00NTRFREF123//BANKREF".to_string(),
            value_date: "230101".to_string(),
            entry_date: Some("0102".to_string()),
            dc_mark: 'C',
            funds_code: None,
            amount: "100,00".to_string(),
            transaction_type: Some("NTRF".to_string()),
            customer_reference: Some("REF123".to_string()),
            bank_reference: Some("BANKREF".to_string()),
            extra_details: Some("EXTRA".to_string()),
            info: Mt940EntryInfo {
                lines: vec!["Line1".to_string(), "Line2".to_string()],
            },
        };

        let desc = build_description(&entry);

        assert_eq!(desc, "NTRF | REF123 | //BANKREF | EXTRA | Line1 Line2");

        // если всё убрать, должен вернуться raw_61
        entry.transaction_type = None;
        entry.customer_reference = None;
        entry.bank_reference = None;
        entry.extra_details = None;
        entry.info.lines.clear();

        let desc2 = build_description(&entry);
        assert_eq!(desc2, entry.raw_61);
    }

    // extract_counterparty_from_mt940

    #[test]
    fn extract_counterparty_prefers_info_lines() {
        let entry = Mt940Entry {
            raw_61: String::new(),
            value_date: "230101".to_string(),
            entry_date: None,
            dc_mark: 'C',
            funds_code: None,
            amount: "10,00".to_string(),
            transaction_type: None,
            customer_reference: None,
            bank_reference: None,
            extra_details: None,
            info: Mt940EntryInfo {
                lines: vec![
                    "Some text".to_string(),
                    "DE89370400440532013000 JOHN DOE".to_string(),
                ],
            },
        };

        let (cp, name) = extract_counterparty_from_mt940(&entry);

        assert_eq!(cp.as_deref(), Some("DE89370400440532013000"));
        assert!(name.is_some());
    }

    #[test]
    fn extract_counterparty_uses_customer_reference_if_no_info_lines() {
        let entry = Mt940Entry {
            raw_61: String::new(),
            value_date: "230101".to_string(),
            entry_date: None,
            dc_mark: 'C',
            funds_code: None,
            amount: "10,00".to_string(),
            transaction_type: None,
            customer_reference: Some("PAYMENT DE89370400440532013000 JOHN DOE".to_string()),
            bank_reference: None,
            extra_details: None,
            info: Mt940EntryInfo { lines: vec![] },
        };

        let (cp, name) = extract_counterparty_from_mt940(&entry);

        assert_eq!(cp.as_deref(), Some("DE89370400440532013000"));
        assert!(name.is_some());
    }

    #[test]
    fn extract_counterparty_returns_none_when_not_found() {
        let entry = Mt940Entry {
            raw_61: String::new(),
            value_date: "230101".to_string(),
            entry_date: None,
            dc_mark: 'C',
            funds_code: None,
            amount: "10,00".to_string(),
            transaction_type: None,
            customer_reference: Some("NO_IBAN_HERE".to_string()),
            bank_reference: None,
            extra_details: None,
            info: Mt940EntryInfo {
                lines: vec!["Just text".to_string()],
            },
        };

        let (cp, name) = extract_counterparty_from_mt940(&entry);

        assert!(cp.is_none());
        assert!(name.is_none());
    }

    // TryFrom<&Mt940Entry> for Transaction

    #[test]
    fn mt940_entry_to_transaction_credit() {
        let entry = Mt940Entry {
            raw_61: ":61:2301010102C100,00".to_string(),
            value_date: "230101".to_string(),
            entry_date: Some("0102".to_string()),
            dc_mark: 'C',
            funds_code: None,
            amount: "100,00".to_string(),
            transaction_type: Some("NTRF".to_string()),
            customer_reference: Some("REF".to_string()),
            bank_reference: None,
            extra_details: None,
            info: Mt940EntryInfo {
                lines: vec!["Desc".to_string()],
            },
        };

        let tx = Transaction::try_from(&entry).unwrap();

        assert_eq!(tx.direction, Direction::Credit);
        assert_eq!(tx.amount, 10_000);

        // value_date = 230101
        assert_eq!(
            tx.value_date,
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap())
        );

        assert_eq!(
            tx.booking_date,
            NaiveDate::from_ymd_opt(2023, 1, 2).unwrap()
        );

        assert!(!tx.description.is_empty());
    }

    #[test]
    fn mt940_entry_to_transaction_debit() {
        let entry = Mt940Entry {
            raw_61: ":61:230101D50,00".to_string(),
            value_date: "230101".to_string(),
            entry_date: None,
            dc_mark: 'D',
            funds_code: None,
            amount: "50,00".to_string(),
            transaction_type: None,
            customer_reference: None,
            bank_reference: None,
            extra_details: None,
            info: Mt940EntryInfo { lines: vec![] },
        };

        let tx = Transaction::try_from(&entry).unwrap();

        assert_eq!(tx.direction, Direction::Debit);
        assert_eq!(tx.amount, 5_000);
    }

    #[test]
    fn mt940_entry_to_transaction_errors_on_unknown_direction() {
        let entry = Mt940Entry {
            raw_61: ":61:230101X100,00".to_string(),
            value_date: "230101".to_string(),
            entry_date: None,
            dc_mark: 'X',
            funds_code: None,
            amount: "100,00".to_string(),
            transaction_type: None,
            customer_reference: None,
            bank_reference: None,
            extra_details: None,
            info: Mt940EntryInfo { lines: vec![] },
        };

        let err = Transaction::try_from(&entry).unwrap_err();
        match err {
            ParseError::InvalidAmount(msg) => {
                assert!(msg.contains("unknown direction"), "unexpected msg: {msg}");
            }
            other => panic!("expected InvalidAmount, got {other:?}"),
        }
    }

    // Mt940Message::from_string_lines

    #[test]
    fn mt940_message_from_string_lines_parses_basic_message() {
        let lines = vec![
            ":20:REF123".to_string(),
            ":25:DE11112222333344445555".to_string(),
            ":28C:1/1".to_string(),
            ":60F:C230101EUR100,00".to_string(),
            ":61:2301020102C50,00NTRFREF//BANK".to_string(),
            ":86:Payment text".to_string(),
            ":62F:C230103EUR150,00".to_string(),
        ];

        let msg = Mt940Message::from_string_lines(&lines).unwrap();

        assert_eq!(msg.transaction_reference.as_deref(), Some("REF123"));
        assert_eq!(msg.account_id, "DE11112222333344445555");
        assert_eq!(msg.statement_number.as_deref(), Some("1/1"));

        assert_eq!(msg.opening_balance.dc_mark, 'C');
        assert_eq!(msg.opening_balance.date, "230101");
        assert_eq!(msg.opening_balance.currency, "EUR");
        assert_eq!(msg.opening_balance.amount, "100,00");

        assert_eq!(msg.entries.len(), 1);
        assert!(msg.closing_balance.is_some());
    }

    #[test]
    fn mt940_message_from_string_lines_requires_account_and_opening_balance() {
        let lines_missing_25 = vec![":20:REF".to_string(), ":60F:C230101EUR100,00".to_string()];

        let err = Mt940Message::from_string_lines(&lines_missing_25).unwrap_err();
        match err {
            ParseError::BadInput(msg) => {
                assert!(msg.contains("missing :25"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadInput, got {other:?}"),
        }

        let lines_missing_60 = vec![":20:REF".to_string(), ":25:ACC".to_string()];

        let err = Mt940Message::from_string_lines(&lines_missing_60).unwrap_err();
        match err {
            ParseError::BadInput(msg) => {
                assert!(
                    msg.contains("missing opening balance"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected BadInput, got {other:?}"),
        }
    }

    // TryFrom<Mt940Message> for Statement

    #[test]
    fn mt940_message_to_statement_maps_basic_fields() {
        let lines = vec![
            ":20:REF123".to_string(),
            ":25:DE11112222333344445555".to_string(),
            ":60F:C230101EUR100,00".to_string(),
            ":61:2301020102C50,00NTRFREF//BANK".to_string(),
            ":62F:D230103EUR80,00".to_string(),
        ];

        let msg = Mt940Message::from_string_lines(&lines).unwrap();
        let stmt = Statement::try_from(msg).unwrap();

        assert_eq!(stmt.account_id, "DE11112222333344445555");
        assert_eq!(stmt.account_name, None);
        assert_eq!(stmt.currency, Currency::EUR);

        // opening: C 100,00 -> +10000
        assert_eq!(stmt.opening_balance, Some(10_000));

        // closing: D 80,00 -> -8000
        assert_eq!(stmt.closing_balance, Some(-8_000));

        assert_eq!(stmt.transactions.len(), 1);

        // period_from по дате opening_balance
        assert_eq!(
            stmt.period_from,
            NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()
        );
        // period_until по дате закрывающего баланса
        assert_eq!(
            stmt.period_until,
            NaiveDate::from_ymd_opt(2023, 1, 3).unwrap()
        );
    }

    #[test]
    fn mt940_message_to_statement_errors_on_unknown_dc_mark_in_balances() {
        let lines = vec![":25:ACC".to_string(), ":60F:X230101EUR100,00".to_string()];

        let msg = Mt940Message::from_string_lines(&lines).unwrap();
        let err = Statement::try_from(msg).unwrap_err();

        match err {
            ParseError::InvalidAmount(msg) => {
                assert!(
                    msg.contains("unknown opening balance direction"),
                    "unexpected msg: {msg}"
                );
            }
            other => panic!("expected InvalidAmount, got {other:?}"),
        }
    }

    #[test]
    fn mt940_message_to_statement_opening_debit_becomes_negative() {
        let lines = vec![
            ":20:REF".to_string(),
            ":25:DE11112222333344445555".to_string(),
            // D -> дебетовый открывающий
            ":60F:D230101EUR100,00".to_string(),
        ];

        let msg = Mt940Message::from_string_lines(&lines).unwrap();
        let stmt = Statement::try_from(msg).unwrap();

        assert_eq!(stmt.opening_balance, Some(-10_000));
    }

    // Mt940Data::parse & TryFrom<Mt940Data>

    #[test]
    fn mt940_data_parse_parses_single_block_curly() {
        let input = r#"{1:F01FOOBARBAXXX0000000000}
        {2:O940...}
        {4:
        :20:REF123
        :25:DE11112222333344445555
        :60F:C230101EUR100,00
        :61:2301020102C50,00NTRFREF//BANK
        :62F:C230103EUR150,00
        -}
        "#;

        let data = Mt940Data::parse(input.as_bytes()).unwrap();
        let stmt = Statement::try_from(data).unwrap();

        assert_eq!(stmt.account_id, "DE11112222333344445555");
        assert_eq!(stmt.currency, Currency::EUR);
        assert_eq!(stmt.opening_balance, Some(10_000));
        assert_eq!(stmt.closing_balance, Some(15_000));
        assert_eq!(stmt.transactions.len(), 1);
    }

    #[test]
    fn mt940_data_parse_errors_on_empty_input() {
        let err = Mt940Data::parse("".as_bytes()).unwrap_err();
        match err {
            ParseError::BadInput(msg) => {
                assert!(msg.contains("0 mt940 messages"), "unexpected msg: {msg}");
            }
            other => panic!("expected BadInput, got {other:?}"),
        }
    }
}
