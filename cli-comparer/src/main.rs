use clap::{Parser, ValueEnum};
use parser::{Camt053Data, CsvData, Mt940Data, ParseError, Statement};
use std::fmt::Display;
use std::fs::File;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process;

#[derive(Parser, Debug)]
#[command(
    name = "cli_comparer",
    version,
    about = "Сравнивает две выписки стандартизированных форматов.",
    long_about = None,
)]
struct Args {
    /// Входной файл 1
    #[arg(long)]
    file1: PathBuf,

    /// Формат входного файла 1
    #[arg(long, value_enum)]
    format1: Format,

    /// Входной файл 2
    #[arg(long)]
    file2: PathBuf,

    /// Формат входного файла 2
    #[arg(long, value_enum)]
    format2: Format,
}

/// Поддерживаемые форматы для CLI
#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    Csv,
    Camt053,
    Mt940,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}

fn parse_to_statement<R: Read>(input_format: &Format, reader: R) -> Result<Statement, ParseError> {
    // парсинг в общую структуру
    match input_format {
        Format::Csv => {
            let data = CsvData::parse(reader)?;
            Statement::try_from(data)
        }
        Format::Camt053 => {
            let data = Camt053Data::parse(reader)?;
            Statement::try_from(data)
        }
        Format::Mt940 => {
            let data = Mt940Data::parse(reader)?;
            Statement::try_from(data)
        }
    }
}

fn print_diff<T>(field: &str, a: &T, b: &T)
where
    T: Display + ?Sized,
{
    println!("Несовпадение {field}");
    println!("  file1: {a}");
    println!("  file2: {b}");
}

fn compare_transactions(a: &Statement, b: &Statement) -> bool {
    let mut eq = true;

    let len_a = a.transactions.len();
    let len_b = b.transactions.len();
    let max_len = len_a.max(len_b);

    for i in 0..max_len {
        let tx_a = a.transactions.get(i);
        let tx_b = b.transactions.get(i);

        match (tx_a, tx_b) {
            (Some(ta), Some(tb)) => {
                if ta != tb {
                    print_diff("transaction", ta, tb);
                    eq = false;
                }
            }
            (Some(ta), None) => {
                println!("Лишняя транзакция в file1 на позиции {i}: {ta}",);
                eq = false;
            }
            (None, Some(tb)) => {
                println!("Лишняя транзакция в file2 на позиции {i}: {tb}");
                eq = false;
            }
            (None, None) => unreachable!("И там, и там None при i < max_len"),
        }
    }
    eq
}

fn compare_statements(a: &Statement, b: &Statement) {
    let mut eq = true;
    if a.account_id != b.account_id {
        eq = false;
        print_diff("account id", a.account_id.as_str(), b.account_id.as_str());
    }

    eq = compare_transactions(a, b) && eq;

    if eq {
        println!("statements are equal")
    }
}

fn run() -> Result<(), ParseError> {
    let args = Args::parse();

    if !args.file1.exists() {
        eprintln!("input file 1 does not exist: {}", args.file1.display());
        process::exit(1)
    }

    if !args.file2.exists() {
        eprintln!("input file 2 does not exist: {}", args.file2.display());
        process::exit(1)
    }

    let file1 = File::open(&args.file1).unwrap_or_else(|err| {
        eprintln!(
            "failed to open input file 1 {}: {err}",
            args.file1.display()
        );
        process::exit(1);
    });

    let file2 = File::open(&args.file2).unwrap_or_else(|err| {
        eprintln!(
            "failed to open input file 2 {}: {err}",
            args.file2.display()
        );
        process::exit(1);
    });

    let reader1 = io::BufReader::new(file1);
    let reader2 = io::BufReader::new(file2);

    let statement1 = parse_to_statement(&args.format1, reader1)?;
    let statement2 = parse_to_statement(&args.format2, reader2)?;

    compare_statements(&statement1, &statement2);

    Ok(())
}
