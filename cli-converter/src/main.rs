use std::path::PathBuf;
use clap::{Parser, ValueEnum};
use parser::{Camt053Data, CsvData, Mt940Data, ParseError, Statement};
use std::fs::File;
use std::process;
use std::io::{self, Write};


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

    /// Если указан, вывод будет записан в указанный файл вместо stdout
    #[arg(long)]
    to_file: Option<PathBuf>,
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

fn write_output<W: Write>(
    statement: &Statement,
    output_format: Format,
    writer: W,
) -> Result<(), ParseError> {
    match output_format {
        Format::Csv => statement.write_csv(writer)?,
        Format::Camt053 => statement.write_camt053(writer)?,
        Format::Mt940 => statement.write_mt940(writer)?,
    }

    Ok(())
}

fn run() -> Result<(), ParseError> {
    let args = Args::parse();
 
    if !args.input.exists() {
        eprintln!("input file does not exist: {}", args.input.display());
        process::exit(1)
    }
    
    let file = File::open(&args.input).unwrap_or_else(|err| {
        eprintln!("failed to open input file {}: {err}", args.input.display());
        process::exit(1);
    });
    

    let reader = io::BufReader::new(file);

    // парсинг в общую структуру
    let statement: Statement = match args.input_format {
        Format::Csv => {
            let data = CsvData::parse(reader)?;
            Statement::try_from(data)?
        },
        Format::Camt053 => {
            let data = Camt053Data::parse(reader)?;
            Statement::try_from(data)?
        },
        Format::Mt940 => {
            let data = Mt940Data::parse(reader)?;
            Statement::try_from(data)?
        }
    };

     match args.to_file {
        // в файл
        Some(path) => {
            let output_file = File::create(&path).unwrap_or_else(|err| {
                eprintln!("failed to create output file {}: {err}", path.display());
                process::exit(1);
            });

            let writer = io::BufWriter::new(output_file);
            write_output(&statement, args.output_format, writer)?;
        }
        // в терминал
        None => {
            let stdout = io::stdout();
            let handle = stdout.lock();
            write_output(&statement, args.output_format, handle)?;
        }
    }

    Ok(())
}
