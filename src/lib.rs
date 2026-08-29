//! A toy payments engine: it reads transactions from a CSV, applies them to
//! per-client accounts, and reports the resulting account state.
//!
//! # How the pieces fit together
//!
//! One transaction flows through the crate in a straight line, and the modules
//! are worth reading in the order it takes:
//!
//! ```text
//! CSV row --> input --> transaction --> engine --> account --> output --> CSV row
//! ```
//!
//! - [`transaction`] is the vocabulary: what a transaction is, and what the
//!   engine keeps of one so that a later dispute can refer back to it.
//! - [`input`] turns the input file into a lazy stream of those transactions.
//! - [`engine`] decides what each one means — it is where every rule about
//!   deposits, withdrawals, disputes, resolves and chargebacks lives, and where
//!   the cases the specification leaves open are resolved and explained.
//! - [`account`] holds the money. It owns the balances and is the only place
//!   they can move, through checked operations the engine has to go through.
//! - [`output`] renders the resulting accounts as the report.
//!
//! The split is deliberate: the rules of the business live in one module, the
//! arithmetic that protects the balances in another, and the CSV format at the
//! two ends. Each can be read, tested and changed without the others.
//!
//! # Example
//!
//! The three steps the diagram above describes, end to end. Reading is generic
//! over [`std::io::Read`], so a socket serves here just as well as the file the
//! binary is given:
//!
//! ```
//! use rust_coding_challenge::engine::Engine;
//! use rust_coding_challenge::{input, output};
//!
//! let transactions = "type, client, tx, amount\n\
//!                     deposit, 1, 1, 2.0\n\
//!                     withdrawal, 1, 2, 0.5\n";
//!
//! // Applied as they arrive, so the input is never held in memory in full.
//! let mut engine = Engine::new();
//! for transaction in input::read_transactions(transactions.as_bytes()) {
//!     engine.apply(&transaction?);
//! }
//!
//! let mut report = Vec::new();
//! output::write_accounts(&mut report, engine.accounts())?;
//!
//! assert_eq!(
//!     String::from_utf8(report)?,
//!     "client,available,held,total,locked\n\
//!      1,1.5000,0.0000,1.5000,false\n"
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod account;
pub mod engine;
pub mod input;
pub mod output;
pub mod transaction;
