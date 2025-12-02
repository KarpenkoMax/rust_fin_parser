pub mod error;
pub mod model;
pub mod csv_parser;
pub mod mt940;
pub mod camt053;
pub mod serialization;

mod utils;

pub use crate::model::{Statement, Direction, Currency, Balance};
pub use crate::camt053::Camt053Data;
pub use crate::csv_parser::CsvData;
pub use crate::error::ParseError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 4;
        assert_eq!(result, 4);
    }
}
