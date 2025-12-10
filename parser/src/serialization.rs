mod camt053_helpers;
mod common;
mod csv_helpers;
use crate::error::ParseError;
use crate::model::{Direction, Statement};
use chrono::Utc;
use csv::WriterBuilder;
use std::io::Write;
mod mt940_helpers;

use crate::camt053::serde_models::*;
use quick_xml::se::to_utf8_io_writer;

impl Statement {
    /// Записывает выписку в CSV в формате
    pub fn write_csv<W: Write>(&self, writer: W) -> Result<(), ParseError> {
        let mut wtr = WriterBuilder::new().has_headers(false).from_writer(writer);

        // ---- ШАПКА ----

        csv_helpers::write_header(&mut wtr, self)?;

        // ---- ТАБЛИЦА ОПЕРАЦИЙ ----

        // Заголовки
        let mut headers_row = csv_helpers::empty_row();
        headers_row[1] = "Дата проводки".to_string();
        headers_row[4] = "Счет".to_string();
        headers_row[9] = "Сумма по дебету".to_string();
        headers_row[13] = "Сумма по кредиту".to_string();
        headers_row[14] = "№ документа".to_string();
        headers_row[16] = "ВО".to_string();
        headers_row[17] = "Банк (БИК и наименование)".to_string();
        headers_row[20] = "Назначение платежа".to_string();
        wtr.write_record(&headers_row)?;

        // Подзаголовки
        let mut subheaders_row = csv_helpers::empty_row();
        subheaders_row[4] = "Дебет".to_string();
        subheaders_row[8] = "Кредит".to_string();
        wtr.write_record(&subheaders_row)?;

        // ---- ДАННЫЕ ----

        let our_account = &self.account_id;
        let our_name = self.account_name.clone().unwrap_or_default();

        for tx in &self.transactions {
            let mut row = csv_helpers::empty_row();

            // Дата проводки
            row[1] = tx.booking_date.format("%d.%m.%Y").to_string();

            // Блоки дебета/кредита
            let cp_acc = tx.counterparty.clone().unwrap_or_default();
            let cp_name = tx.counterparty_name.clone().unwrap_or_default();

            let (debit_block, credit_block) = match tx.direction {
                Direction::Debit => {
                    let debit = csv_helpers::make_party_block(our_account, &our_name);
                    let credit = csv_helpers::make_party_block(&cp_acc, &cp_name);
                    (debit, credit)
                }
                Direction::Credit => {
                    let debit = csv_helpers::make_party_block(&cp_acc, &cp_name);
                    let credit = csv_helpers::make_party_block(our_account, &our_name);
                    (debit, credit)
                }
            };

            row[4] = debit_block;
            row[8] = credit_block;

            // Суммы
            match tx.direction {
                Direction::Debit => {
                    row[9] = common::format_minor_units(tx.amount, '.');
                }
                Direction::Credit => {
                    row[13] = common::format_minor_units(tx.amount, '.');
                }
            }

            // Назначение платежа
            row[20] = tx.description.clone();

            wtr.write_record(&row)?;
        }

        // ---- Footer ----
        csv_helpers::write_footer(&mut wtr, self)?;

        wtr.flush()?;
        Ok(())
    }

    /// Записывает выписку в формате CAMT.053 (XML)
    pub fn write_camt053<W: Write>(&self, writer: W) -> Result<(), ParseError> {
        let now = Utc::now();
        let ccy_code = camt053_helpers::currency_code(&self.currency);

        // Собираем Statement
        let mut stmt = Camt053Statement::default();

        stmt.id = Some(format!(
            "stmt-{}-{}",
            self.account_id,
            now.format("%Y%m%d%H%M%S")
        ));

        stmt.sequence_number = Some(1);

        stmt.created_at = Some(now.format("%Y-%m-%dT%H:%M:%S").to_string());
        stmt.period = Some(Camt053Period {
            from: Some(camt053_helpers::format_iso_date(self.period_from)),
            to: Some(camt053_helpers::format_iso_date(self.period_until)),
        });
        stmt.account = Camt053Account {
            id: Camt053AccountId {
                iban: Some(self.account_id.clone()),
            },
            name: self.account_name.clone(),
            currency: Some(ccy_code.to_string()),
        };
        stmt.balances = camt053_helpers::balances_from_statement(self, ccy_code);
        stmt.entries = camt053_helpers::entries_from_transactions(&self.transactions, ccy_code);

        // Заворачиваем в Document
        let doc = Camt053Document {
            bank_to_customer: Camt053BankToCustomer {
                group_header: Some(Camt053GroupHeader {
                    message_id: format!("serialized_via_parser-{}", now.format("%Y%m%d%H%M%S")),
                    created_at: Some(now.format("%Y-%m-%dT%H:%M:%S").to_string()),
                }),
                statements: vec![stmt],
            },
        };

        to_utf8_io_writer(writer, &doc)?;
        Ok(())
    }

    /// Записывает выписку в формате MT940
    pub fn write_mt940<W: Write>(&self, mut writer: W) -> Result<(), ParseError> {
        writeln!(writer, "{{4:")?;

        // ---- Заголовочные теги ----

        // :20: Transaction Reference - плейсхолдер
        writeln!(writer, ":20:SERIALIZED")?;

        // :25: Account Identification - наш счёт
        writeln!(writer, ":25:{}", self.account_id)?;

        // :28C: Statement Number - плейсхолдер "1/1"
        writeln!(writer, ":28C:1/1")?;

        // ---- :60F: Opening Balance ----

        let ccy_code = mt940_helpers::currency_code(&self.currency);

        let opening_minor: i128 = self.opening_balance.unwrap_or(0);
        let (opening_dc, opening_abs) = if opening_minor >= 0 {
            ('C', opening_minor)
        } else {
            ('D', -opening_minor)
        };
        let opening_abs_u = opening_abs as u64;
        let opening_amount_str = common::format_minor_units(opening_abs_u, ',');

        let opening_date_str = mt940_helpers::format_yymmdd(self.period_from);

        writeln!(
            writer,
            ":60F:{opening_dc}{opening_date_str}{ccy_code}{opening_amount_str}"
        )?;

        // ---- :61: / :86: Transactions ----

        for tx in &self.transactions {
            let line_61 = mt940_helpers::format_61_line(tx);
            writeln!(writer, ":61:{line_61}")?;

            if let Some(info) = mt940_helpers::format_86_line(tx) {
                writeln!(writer, ":86:{info}")?;
            }
        }

        // ---- :62F: Closing Balance ----

        if let Some(closing_minor) = self.closing_balance {
            let (closing_dc, closing_abs) = if closing_minor >= 0 {
                ('C', closing_minor)
            } else {
                ('D', -closing_minor)
            };
            let closing_abs_u = closing_abs as u64;
            let closing_amount_str = common::format_minor_units(closing_abs_u, ',');

            let closing_date_str = mt940_helpers::format_yymmdd(self.period_until);

            writeln!(
                writer,
                ":62F:{closing_dc}{closing_date_str}{ccy_code}{closing_amount_str}"
            )?;
        }

        // Закрываем блок 4
        writeln!(writer, "-}}")?;

        Ok(())
    }
}
