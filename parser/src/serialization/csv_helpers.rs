use chrono::Datelike;
use crate::model::Currency;


const COLS: usize = 23;


pub(crate) fn empty_row() -> Vec<String> {
    vec![String::new(); COLS]
}

pub(crate) fn format_rus_date(d: chrono::NaiveDate) -> String {
    let day = d.day();
    let year = d.year();
    let month = match d.month() {
        1 => "января",
        2 => "февраля",
        3 => "марта",
        4 => "апреля",
        5 => "мая",
        6 => "июня",
        7 => "июля",
        8 => "августа",
        9 => "сентября",
        10 => "октября",
        11 => "ноября",
        12 => "декабря",
        _ => unreachable!(),
    };
    format!("{day:02} {month} {year} г.")
}

pub(crate) fn currency_label(cur: &Currency) -> String {
    match cur {
        Currency::RUB => "Российский рубль".to_string(),
        Currency::EUR => "Евро".to_string(),
        Currency::USD => "Доллар США".to_string(),
        Currency::CNY => "Китайский юань".to_string(),
        Currency::Other(s) => s.clone(),
    }
}

pub(crate) fn format_amount(amount: u64) -> String {
    let v = amount; // если amount уже в "копейках" - оставляем так
    let rub = v / 100;
    let kop = v % 100;
    format!("{rub}.{kop:02}")
}

// Блок с реквизитами стороны: делаем 3 непустые строки,
// чтобы extract_account_and_name мог взять 0-ю как счёт, 2-ю как имя.
pub(crate) fn make_party_block(account: &str, name: &str) -> String {
    if account.is_empty() && name.is_empty() {
        return String::new();
    }
    let name_line = if name.is_empty() { "-" } else { name };
    format!("{account}\n-\n{name_line}")
}