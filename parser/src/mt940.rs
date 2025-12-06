mod utils;
use chrono::NaiveDate;
use std::io::{BufRead, BufReader, Read};
use crate::error::ParseError;
use crate::model::{Direction, Statement, Transaction, Currency, Balance};
use crate::utils::{parse_amount, parse_currency};
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
        let mut tx_ref: Option<String> = None;  // :20:
        let mut account_id: Option<String> = None;  // :25:
        let mut statement_number: Option<String> = None;  // :28C:

        let mut opening_balance: Option<Mt940Balance> = None;  // :60F: / :60M:
        let mut closing_balance: Option<Mt940Balance> = None;  // :62F:
        let mut closing_available_balance: Option<Mt940Balance> = None;  // :64:

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
                        }
                    }
                    "62F" => {
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
                        current_entry = Some(Mt940Entry::from_61_line(
                            value,
                            line_trimmed.to_string(),
                        )?);
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
        let opening_balance = opening_balance
            .ok_or_else(|| ParseError::BadInput("MT940: missing opening balance :60F:/:60M:".into()))?;

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
    if let Some(ref cref) = entry.customer_reference {
        if let Some((iban, name)) = find_iban_and_name_in_line(cref) {
            return (Some(iban), name);
        }
    }

    // Пробуем bank_reference
    if let Some(ref bref) = entry.bank_reference {
        if let Some((iban, name)) = find_iban_and_name_in_line(bref) {
            return (Some(iban), name);
        }
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
        let booking_date = derive_booking_date(value_date.clone(), entry.entry_date.as_deref())?;

        let description = build_description(&entry);
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
        if len >= idx + 4
            && value[idx..idx + 4]
                .chars()
                .all(|c| c.is_ascii_digit())
        {
            entry_date = Some(value[idx..idx + 4].to_string());
            idx += 4;
        }

        // D/C
        if idx >= len {
            return Err(ParseError::BadInput(format!(
                "no debit/credit mark in :61: '{value}'"
            )));
        }
        let dc_mark = value[idx..].chars().next().unwrap();
        idx += dc_mark.len_utf8();

        // optional funds code (например R в "DR")
        let mut funds_code = None;
        if idx < len {
            let c = value[idx..].chars().next().unwrap();
            // очень упрощенно: если буква и не цифра/знак суммы - считаем funds code
            if c.is_ascii_alphabetic() && c != 'C' && c != 'D' {
                funds_code = Some(c);
                idx += c.len_utf8();
            }
        }

        // amount: до первого символа, который не цифра, не ',' и не '.'
        let mut amount_start = idx;
        while amount_start < len {
            let ch = value[amount_start..].chars().next().unwrap();
            if ch.is_ascii_digit() || ch == ',' || ch == '.' {
                break;
            }
            amount_start += ch.len_utf8();
        }
        idx = amount_start;

        while idx < len {
            let ch = value[idx..].chars().next().unwrap();
            if ch.is_ascii_digit() || ch == ',' || ch == '.' {
                idx += ch.len_utf8();
            } else {
                break;
            }
        }

        if idx <= amount_start {
            return Err(ParseError::BadInput(format!(
                "no amount found in :61: '{value}'"
            )));
        }

        let amount = value[amount_start..idx].to_string();

        // transaction_type / references / extra
        let mut rest = value[idx..].trim_start();
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

#[derive(Debug, Clone)]
pub struct Mt940Data {
    /// Пока один Statement
    pub message: Mt940Message,
}

impl Mt940Data {
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
        Ok(
            Statement::try_from(data.message)?
        )
    }
}