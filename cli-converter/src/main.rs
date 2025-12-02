use std::path::PathBuf;
use clap::{Parser, ValueEnum};
use parser::{Camt053Data, CsvData, ParseError, Statement};
use std::fs::{self, File};
use std::io;
use std::process;


#[derive(Parser, Debug)]
#[command(
    name = "cli_converter",
    version,
    about = "Конвертирует выписки между различными стандартизированными форматами.",
    long_about = None,
)]
struct Args {
    /// Входной файл
    #[arg(long)]
    input: PathBuf,

    /// Формат входного файла
    #[arg(long, value_enum)]
    input_format: Format,

    /// Формат выходного файла
    #[arg(long, value_enum)]
    output_format: Format,
}

/// Поддерживаемые форматы для CLI
#[derive(Copy, Clone, Debug, ValueEnum)]
enum Format {
    Csv,
    Camt053,
    // Mt940,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err}");
        process::exit(1);
    }
}

fn run() -> Result<(), ParseError> {
    let args = Args::parse();

    println!("{args:#?}");
 
    if !args.input.exists() {
        eprintln!("input file does not exist: {}", args.input.display());
        process::exit(1)
    }
    
    let file = File::open(&args.input).unwrap_or_else(|err| {
        eprintln!("failed to open input file {}: {err}", args.input.display());
        process::exit(1);
    });
    

    let reader = io::BufReader::new(file);

    let statement: Statement = match args.input_format {
        Format::Csv => {
            let data = CsvData::parse(reader)?;
            Statement::try_from(data)?
        },
        Format::Camt053 => {
            let data = Camt053Data::parse(reader)?;
            Statement::try_from(data)?
        },
    };

    let stdout = io::stdout();
    let handle = stdout.lock();

    match args.output_format {
        Format::Csv => {
            statement.write_csv(handle)?;
        }
        Format::Camt053 => {
            statement.write_camt053(handle)?;
        }
    }

    Ok(())
}
