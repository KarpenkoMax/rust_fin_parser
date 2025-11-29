
use chrono::NaiveDate;

pub type Balance = i128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Currency {
    RUB,
    EUR,
    USD,
    CNY,
    Other(String),
}

#[derive(Debug)]
pub struct Statement {
    account_id: String,
    account_name: Option<String>,
    currency: Currency,
    opening_balance: Option<Balance>,
    closing_balance: Option<Balance>,
    transactions: Vec<Transaction>,
    period_from: NaiveDate,
    period_until: NaiveDate,
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

#[derive(Debug)]
pub struct Transaction {
    booking_date: NaiveDate,
    value_date: Option<NaiveDate>,
    amount: u64,
    direction: Direction,
    description: String,
    counterparty: Option<String>,
}


impl Transaction {
    pub fn new(
        booking_date: NaiveDate,
        value_date: Option<NaiveDate>,
        amount: u64,
        direction: Direction,
        description: String,
        counterparty: Option<String>,
    ) -> Self {
        Transaction {
            booking_date,
            value_date,
            amount,
            direction,
            description,
            counterparty,
        }
    }
}
