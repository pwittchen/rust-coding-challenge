//! A toy payments engine: it reads transactions from a CSV, applies them to
//! per-client accounts, and reports the resulting account state.

pub mod input;
pub mod transaction;
