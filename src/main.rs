use std::env;
use std::error::Error;
use std::ffi::OsString;
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
    let path = input_path(env::args_os().skip(1))?;

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
///
/// A second argument is rejected rather than ignored: it means the invocation
/// does not say what it looks like it says, and silently dropping it could send
/// the report of the wrong file downstream.
fn input_path<I: IntoIterator<Item = OsString>>(arguments: I) -> Result<PathBuf, Box<dyn Error>> {
    const USAGE: &str = "expected the input CSV as the only argument";

    let mut arguments = arguments.into_iter();
    let path = arguments.next().ok_or(USAGE)?;

    if arguments.next().is_some() {
        return Err(USAGE.into());
    }

    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input_path_of(arguments: &[&str]) -> Result<PathBuf, Box<dyn Error>> {
        input_path(arguments.iter().map(OsString::from))
    }

    #[test]
    fn takes_the_input_file_from_the_only_argument() {
        let path = input_path_of(&["transactions.csv"]).expect("one argument is enough");

        assert_eq!(path, PathBuf::from("transactions.csv"));
    }

    #[test]
    fn rejects_an_invocation_that_does_not_name_exactly_one_file() {
        assert!(input_path_of(&[]).is_err(), "no argument");
        assert!(
            input_path_of(&["transactions.csv", "accounts.csv"]).is_err(),
            "two arguments"
        );
    }
}
