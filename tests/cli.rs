//! End-to-end tests of the compiled binary.
//!
//! The unit tests cover what the engine does with a transaction; these cover
//! what the program does with a *run*: which stream each result goes to, and
//! what the exit status says about it. That contract cannot be observed from
//! inside the crate, because it is made of the process's stdout, stderr and
//! status code, so it is exercised here by running the binary itself.

// Clippy's exemption for panicking in tests recognises `#[test]` functions but
// not the helpers they share, which are test code just the same: a fixture that
// cannot be set up has no result to report other than a failed test. The
// panic-free rule that matters — the one covering everything that runs in
// production — is denied package-wide in `Cargo.toml`.
#![allow(clippy::expect_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The binary built from this crate, wired up by Cargo.
const BINARY: &str = env!("CARGO_BIN_EXE_rust-coding-challenge");

/// Runs the binary with `arguments` and captures the result.
fn run(arguments: &[&Path]) -> Output {
    Command::new(BINARY)
        .args(arguments)
        .output()
        .expect("the binary should be runnable")
}

/// Runs the binary against an input file holding `contents`.
///
/// The file is named after the calling test so that tests running in parallel
/// cannot tread on each other's input, and is removed once the run is over.
fn run_on_input(name: &str, contents: &str) -> Output {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.csv"));
    fs::write(&path, contents).expect("the input file should be writable");

    let output = run(&[&path]);

    fs::remove_file(&path).expect("the input file should be removable");

    output
}

fn stdout_of(output: &Output) -> &str {
    str::from_utf8(&output.stdout).expect("stdout should be valid UTF-8")
}

fn stderr_of(output: &Output) -> &str {
    str::from_utf8(&output.stderr).expect("stderr should be valid UTF-8")
}

#[test]
fn writes_the_report_to_stdout_and_succeeds() {
    let output = run_on_input(
        "well-formed",
        "type, client, tx, amount\n\
         deposit, 1, 1, 1.0\n\
         deposit, 2, 2, 2.0\n\
         withdrawal, 1, 3, 1.5\n",
    );

    assert!(output.status.success(), "a readable input should succeed");
    assert_eq!(
        stdout_of(&output),
        "client,available,held,total,locked\n\
         1,1.0000,0.0000,1.0000,false\n\
         2,2.0000,0.0000,2.0000,false\n"
    );
    assert_eq!(stderr_of(&output), "", "a successful run says nothing");
}

#[test]
fn succeeds_on_an_input_that_holds_no_transactions() {
    let output = run_on_input("no-rows", "type, client, tx, amount\n");

    assert!(output.status.success(), "an empty input is not an error");
    assert_eq!(
        stdout_of(&output),
        "client,available,held,total,locked\n",
        "the report is a well-formed CSV even with nothing to report"
    );
}

#[test]
fn reports_a_malformed_record_and_writes_no_report() {
    let output = run_on_input(
        "malformed",
        "type, client, tx, amount\n\
         deposit, 1, 1, 1.0\n\
         teleport, 1, 2, 1.0\n",
    );

    assert!(!output.status.success(), "a malformed record fails the run");
    assert_eq!(
        stdout_of(&output),
        "",
        "a run that fails writes no report at all, not a partial one"
    );

    // The record the reader rejects is named, and so is the file it came from:
    // the error has to be actionable without re-running the program.
    let stderr = stderr_of(&output);
    assert!(stderr.contains("malformed.csv"), "stderr: {stderr}");
    assert!(stderr.contains("line: 3"), "stderr: {stderr}");
    assert!(stderr.contains("teleport"), "stderr: {stderr}");
}

#[test]
fn reports_an_input_file_that_cannot_be_read() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("does-not-exist.csv");
    let output = run(&[&path]);

    assert!(!output.status.success(), "an unreadable file fails the run");
    assert_eq!(stdout_of(&output), "");
    assert!(
        stderr_of(&output).contains("does-not-exist.csv"),
        "stderr: {}",
        stderr_of(&output)
    );
}

#[test]
fn reports_an_invocation_that_does_not_name_exactly_one_file() {
    let input = Path::new("transactions.csv");

    for arguments in [&[][..], &[input, input][..]] {
        let output = run(arguments);

        assert!(
            !output.status.success(),
            "{arguments:?} should not be accepted"
        );
        assert_eq!(stdout_of(&output), "");
        assert!(!stderr_of(&output).is_empty(), "the usage is explained");
    }
}
