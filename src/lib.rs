//! A toy payments engine: it reads transactions from a CSV, applies them to
//! per-client accounts, and reports the resulting account state.

pub mod account;
pub mod engine;
pub mod input;
pub mod output;
pub mod transaction;
