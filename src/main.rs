use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use rust_coding_challenge::input;
use rust_coding_challenge::transaction::Transaction;

/// Header of the account report, per the required output format.
const ACCOUNT_COLUMNS: [&str; 5] = ["client", "available", "held", "total", "locked"];

fn main() -> ExitCode {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run() -> Result<(), Box<dyn Error>> {
    let path = input_path()?;

    // Errors from the reader name neither the file nor the record, so both are
    // reported here instead.
    let describe = |error: csv::Error| format!("{}: {error}", path.display());

    // The transactions are only loaded for now; applying them to accounts comes
    // next, at which point the stream can be consumed record by record instead
    // of being collected.
    let transactions: Vec<Transaction> = input::read_transactions_from_path(&path)
        .map_err(describe)?
        .collect::<Result<_, _>>()
        .map_err(describe)?;
    eprintln!("loaded {} transactions", transactions.len());

    write_accounts(io::stdout().lock())
}

/// Returns the input file, the first and only argument to the binary.
fn input_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);

    let path = arguments
        .next()
        .ok_or("expected the input CSV as the only argument")?;

    if arguments.next().is_some() {
        return Err("expected the input CSV as the only argument".into());
    }

    Ok(PathBuf::from(path))
}

/// Writes the account report to `writer`.
///
/// Only the header is written so far, since no account state is derived yet.
fn write_accounts<W: Write>(writer: W) -> Result<(), Box<dyn Error>> {
    let mut writer = csv::Writer::from_writer(writer);
    writer.write_record(ACCOUNT_COLUMNS)?;
    writer.flush()?;

    Ok(())
}
