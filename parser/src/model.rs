use chrono::NaiveDate;
use std::fmt;

/// Тип для хранения баланса счёта в "копейках", signed
pub type Balance = i128;

/// Структура с поддерживаемыми валютами
///    
/// Важно:
/// При использовании [`Currency::Other`] не все операции парсинга/сериализации будут давать стабильный результат.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Currency {
    /// Российский рубль
    RUB,
    /// Евро
    EUR,
    /// Американский доллар
    USD,
    /// Китайский юань
    CNY,

    /// Неподдерживаемая валюта
    /// 
    /// Содержится как строка
    /// 
    /// Важно:
    /// При использовании [`Currency::Other`] не все операции парсинга/сериализации будут давать стабильный результат.
    Other(String),
}


/// Центральная/корневая структура библиотеки, содержащая одну банковскую выписку.
/// 
/// При конвертации выписок исходные данные попадают в эту структуру,
/// а уже потом сериализуются в нужный формат.
/// 
/// Пример использования:
/// ```no_run
/// let data = CsvData::parse(reader)?;
/// let statement = Statement::try_from(data)?
/// 
/// let stdout = io::stdout();
/// let writer = stdout.lock();
/// 
/// statement.write_mt940(writer);
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct Statement {
    /// идентификатор счёта
    pub account_id: String,
    /// имя счёта или его владельца в человекочитаемом формате
    pub account_name: Option<String>,
    /// валюта
    pub currency: Currency,

    /// открывающий баланс
    pub opening_balance: Option<Balance>,
    /// закрывающий баланс
    pub closing_balance: Option<Balance>,
    /// транзакции
    pub transactions: Vec<Transaction>,
    /// начало временного периода выписки
    pub period_from: NaiveDate,
    /// конец временного периода выписки
    pub period_until: NaiveDate,
}

impl Statement {
    /// Go to [`Statement`]
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

/// Направление транзакции (Дебет/Кредит)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Дебет
    Debit,
    /// Кредит
    Credit,
}

/// Центральная/корневая структура библиотеки, содержащая одну транзакцию.
/// 
/// При конвертации выписок или транзакций исходные данные попадают в эту структуру.
/// 
/// При обычном использовании библиотеки внешнее взаимодействие с этой структурой не является обязательным,
/// но может быть полезно при необходимости редактирования транзакций уже после парсинга.
#[derive(Debug, PartialEq, Eq)]
pub struct Transaction {
    /// дата проводки
    pub booking_date: NaiveDate,
    /// дата валютирования
    pub value_date: Option<NaiveDate>,
    /// денежная сумма (в "копейках")
    pub amount: u64,
    /// направление транзакции
    pub direction: Direction,
    /// текстовое описание
    pub description: String,
    /// идентификатор контрагента
    pub counterparty: Option<String>,
    /// имя контрагента
    pub counterparty_name: Option<String>,
}


impl Transaction {
    /// Go to [`Transaction`]
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
