mod csv_helpers;
mod camt053_helpers;
mod common;
use std::io::Write;
use chrono::Utc;
use csv::WriterBuilder;
use crate::error::ParseError;
use crate::model::{Statement, Direction};

use quick_xml::se::to_utf8_io_writer;
use crate::camt053::serde_models::*;

impl Statement {
    /// Записывает выписку в CSV в формате
    pub fn write_csv<W: Write>(&self, writer: W) -> Result<(), ParseError> {
        

        let mut wtr = WriterBuilder::new()
            .has_headers(false)
            .from_writer(writer);

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
        stmt.balances = camt053_helpers::balances_from_statement(self, &ccy_code);
        stmt.entries = camt053_helpers::entries_from_transactions(&self.transactions, &ccy_code);

        // Заворачиваем в Document
        let doc = Camt053Document {
            bank_to_customer: Camt053BankToCustomer {
                group_header: Some(Camt053GroupHeader {
                    message_id: format!(
                        "serialized_via_parser-{}",
                        now.format("%Y%m%d%H%M%S")
                    ),
                    created_at: Some(now.format("%Y-%m-%dT%H:%M:%S").to_string()),
                }),
                statements: vec![stmt],
            },
        };

        to_utf8_io_writer(writer, &doc)?;
        Ok(())
    }
}

