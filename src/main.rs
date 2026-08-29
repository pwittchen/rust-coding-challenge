use std::env;
use std::error::Error;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use rust_coding_challenge::engine::Engine;
use rust_coding_challenge::{input, output};

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

    let transactions = input::read_transactions_from_path(&path).map_err(describe)?;

    // The records are applied as they are read, so only the account state and
    // the disputable transactions are ever held in memory.
    let mut engine = Engine::new();
    for transaction in transactions {
        engine.apply(&transaction.map_err(describe)?);
    }

    output::write_accounts(io::stdout().lock(), engine.accounts())?;

    Ok(())
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
