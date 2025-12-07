use chrono::NaiveDate;
use std::fmt;
pub type Balance = i128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Currency {
    RUB,
    EUR,
    USD,
    CNY,
    Other(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Statement {
    pub account_id: String,
    pub account_name: Option<String>,
    pub currency: Currency,
    pub opening_balance: Option<Balance>,
    pub closing_balance: Option<Balance>,
    pub transactions: Vec<Transaction>,
    pub period_from: NaiveDate,
    pub period_until: NaiveDate,
}

impl Statement {
    pub fn new(
        account_id: String,
        account_name: Option<String>,
        currency: Currency,
        opening_balance: Option<Balance>,
        closing_balance: Option<Balance>,
        transactions: Vec<Transaction>,
        period_from: NaiveDate,
        period_until: NaiveDate,
    ) -> Self {
        Statement { 
            account_id,
            account_name,
            currency,
            opening_balance,
            closing_balance,
            transactions,
            period_from,
            period_until,
         }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Debit,
    Credit,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Transaction {
    pub booking_date: NaiveDate,
    pub value_date: Option<NaiveDate>,
    pub amount: u64,
    pub direction: Direction,
    pub description: String,
    // id
    pub counterparty: Option<String>,
    // name
    pub counterparty_name: Option<String>,
}


impl Transaction {
    pub fn new(
        booking_date: NaiveDate,
        value_date: Option<NaiveDate>,
        amount: u64,
        direction: Direction,
        description: String,
        counterparty: Option<String>,
        counterparty_name: Option<String>,
    ) -> Self {
        Transaction {
            booking_date,
            value_date,
            amount,
            direction,
            description,
            counterparty,
            counterparty_name,
        }
    }
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Credit => write!(f, "Credit"),
            Direction::Debit  => write!(f, "Debit"),
        }
    }
}


impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value_date_str = self
            .value_date
            .map(|d| d.to_string())
            .unwrap_or_default();

        let counterparty_str = self
            .counterparty
            .as_deref()
            .unwrap_or("");

        let counterparty_name_str = self
            .counterparty_name
            .as_deref()
            .unwrap_or("");

        write!(
            f,
            "{:<10} {:<10} {:<6} {:>15} {} {} {}",
            self.booking_date,
            value_date_str,
            self.direction,
            self.amount,
            counterparty_str,
            counterparty_name_str,
            self.description,
        )
    }
}
