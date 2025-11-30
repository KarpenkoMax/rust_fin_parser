
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
    pub(crate) account_id: String,
    pub(crate) account_name: Option<String>,
    pub(crate) currency: Currency,
    pub(crate) opening_balance: Option<Balance>,
    pub(crate) closing_balance: Option<Balance>,
    pub(crate) transactions: Vec<Transaction>,
    pub(crate) period_from: NaiveDate,
    pub(crate) period_until: NaiveDate,
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
    pub(crate) booking_date: NaiveDate,
    pub(crate) value_date: Option<NaiveDate>,
    pub(crate) amount: u64,
    pub(crate) direction: Direction,
    pub(crate) description: String,
    // id
    pub(crate) counterparty: Option<String>,
    // name
    pub(crate) counterparty_name: Option<String>,
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
