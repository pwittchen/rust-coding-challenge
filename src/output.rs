//! Writing the resulting account state to CSV.

use std::io::Write;

use serde::Serialize;

use crate::account::Account;
use crate::transaction::{Amount, ClientId, SCALE};

/// One row of the account report.
///
/// A separate type from [`Account`], because the report is a view of an account
/// rather than the account itself: it spells out the total and renders the
/// amounts for display.
#[derive(Debug, Serialize)]
struct AccountReport {
    client: ClientId,
    available: String,
    held: String,
    total: String,
    locked: bool,
}

impl From<&Account> for AccountReport {
    fn from(account: &Account) -> Self {
        Self {
            client: account.client(),
            available: format_amount(account.available()),
            held: format_amount(account.held()),
            total: format_amount(account.total()),
            locked: account.is_locked(),
        }
    }
}

/// Renders an amount with the precision required by the output format.
///
/// The amount is formatted here rather than left to the serializer, which drops
/// trailing zeros, so every balance is reported with the same four decimal
/// places. The engine already holds every balance at this scale, so nothing is
/// lost.
///
/// The digits are laid out by hand rather than by asking the decimal for a fixed
/// precision: given one, its formatter builds the text in a fixed 32-byte buffer
/// and panics when the result does not fit, which it does not for a balance of
/// 28 or more integer digits. Rendering without a precision is sized by the
/// mantissa and cannot overflow, so the fraction is padded here instead.
///
/// A balance that lands on exactly zero is reported as a positive zero: the
/// decimal keeps a sign of its own, so subtracting a zero amount leaves a
/// negative zero behind, and `-0.0000` is not a balance a reader expects.
fn format_amount(value: Amount) -> String {
    let value = if value.is_zero() { Amount::ZERO } else { value };

    let text = value.to_string();
    let (whole, fraction) = text.split_once('.').unwrap_or((text.as_str(), ""));

    let mut formatted = String::from(whole);
    formatted.push('.');

    let mut digits = fraction.chars();
    for _ in 0..SCALE {
        formatted.push(digits.next().unwrap_or('0'));
    }

    formatted
}

/// Writes `accounts` to `writer` as CSV, one row per client.
///
/// The header is always written, so the output is a well-formed CSV even when
/// there are no accounts to report.
///
/// # Errors
///
/// Fails if `writer` refuses a row or cannot be flushed — a closed pipe or a
/// full disk, say.
pub fn write_accounts<'a, W, I>(writer: W, accounts: I) -> csv::Result<()>
where
    W: Write,
    I: IntoIterator<Item = &'a Account>,
{
    // The header is written explicitly rather than derived from the row type, so
    // that it is present even for an empty report.
    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_writer(writer);
    writer.write_record(["client", "available", "held", "total", "locked"])?;

    for account in accounts {
        writer.serialize(AccountReport::from(account))?;
    }

    writer.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Engine;
    use crate::input::read_transactions;

    fn report(input: &str) -> String {
        let mut engine = Engine::new();
        for transaction in read_transactions(input.as_bytes()) {
            engine.apply(&transaction.expect("input should be readable"));
        }

        let mut output = Vec::new();
        write_accounts(&mut output, engine.accounts()).expect("report should be writable");

        String::from_utf8(output).expect("report should be valid UTF-8")
    }

    #[test]
    fn writes_only_the_header_when_there_are_no_accounts() {
        assert_eq!(report(""), "client,available,held,total,locked\n");
    }

    #[test]
    fn writes_the_example_from_the_specification() {
        let report = report(
            "type, client, tx, amount\n\
             deposit, 1, 1, 1.0\n\
             deposit, 2, 2, 2.0\n\
             deposit, 1, 3, 2.0\n\
             withdrawal, 1, 4, 1.5\n\
             withdrawal, 2, 5, 3.0\n",
        );

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             1,1.5000,0.0000,1.5000,false\n\
             2,2.0000,0.0000,2.0000,false\n"
        );
    }

    #[test]
    fn reports_held_funds_and_a_frozen_account() {
        let report = report(
            "type,client,tx,amount\n\
             deposit,1,1,1.5000\n\
             deposit,1,2,2.0\n\
             dispute,1,1,\n\
             deposit,2,3,1.0\n\
             dispute,2,3,\n\
             chargeback,2,3,\n",
        );

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             1,2.0000,1.5000,3.5000,false\n\
             2,0.0000,0.0000,0.0000,true\n"
        );
    }

    #[test]
    fn reports_negative_balances() {
        let report = report(
            "type,client,tx,amount\n\
             deposit,1,1,1.5\n\
             withdrawal,1,2,1.5\n\
             dispute,1,1,\n\
             chargeback,1,1,\n",
        );

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             1,-1.5000,0.0000,-1.5000,true\n"
        );
    }

    #[test]
    fn reports_an_account_whose_transactions_were_all_rejected() {
        let report = report("type,client,tx,amount\nwithdrawal,9,1,1.0\n");

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             9,0.0000,0.0000,0.0000,false\n"
        );
    }

    #[test]
    fn reports_amounts_with_a_precision_of_four_decimal_places() {
        let report = report(
            "type,client,tx,amount\n\
             deposit,1,1,0.0001\n\
             deposit,1,2,10\n\
             deposit,2,3,0.00004\n",
        );

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             1,10.0001,0.0000,10.0001,false\n\
             2,0.0000,0.0000,0.0000,false\n"
        );
    }

    #[test]
    fn reports_available_and_held_that_add_up_to_the_reported_total() {
        // Amounts finer than the report can show are cut on the way in, so no
        // balance can carry a fraction that the report counts in the total but
        // cannot show in the column it came from.
        let report = report(
            "type,client,tx,amount\n\
             deposit,1,1,0.00005\n\
             deposit,1,2,0.00005\n\
             dispute,1,2,\n",
        );

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             1,0.0000,0.0000,0.0000,false\n"
        );
    }

    #[test]
    fn reports_a_zero_balance_without_a_negative_sign() {
        assert_eq!(format_amount(Amount::ZERO), "0.0000");
        assert_eq!(format_amount(-Amount::ZERO), "0.0000");
    }

    #[test]
    fn reports_the_largest_balance_an_account_can_hold() {
        // `src/account.rs` proves an account accepts a deposit of `Amount::MAX`,
        // so the report has to be able to print one: a balance the engine can
        // store but not render would be a panic waiting on the last line of a
        // successful run.
        assert_eq!(
            format_amount(Amount::MAX),
            "79228162514264337593543950335.0000"
        );
        assert_eq!(
            format_amount(-Amount::MAX),
            "-79228162514264337593543950335.0000"
        );
    }

    #[test]
    fn reports_a_balance_whose_digits_exceed_a_fixed_width_buffer() {
        // Twenty-eight integer digits and four decimals do not fit the buffer
        // the decimal's own formatter uses when it is given a precision, which
        // is why `format_amount` lays the digits out itself.
        let balance: Amount = "1234567890123456789012345678"
            .parse()
            .expect("valid decimal");

        assert_eq!(format_amount(balance), "1234567890123456789012345678.0000");
    }

    #[test]
    fn reports_a_large_balance_driven_negative_by_a_chargeback() {
        // The same boundary reached through the engine rather than by hand, and
        // on the sign that adds a character to the rendered balance.
        let report = report(
            "type,client,tx,amount\n\
             deposit,1,1,1234567890123456789012345678\n\
             withdrawal,1,2,1234567890123456789012345678\n\
             dispute,1,1,\n\
             chargeback,1,1,\n",
        );

        assert_eq!(
            report,
            "client,available,held,total,locked\n\
             1,-1234567890123456789012345678.0000,0.0000,-1234567890123456789012345678.0000,true\n"
        );
    }

    #[test]
    fn cuts_a_fraction_finer_than_the_reported_scale() {
        // Balances reach the report already cut to scale, so this only guards
        // the renderer itself against ever widening a row.
        let balance: Amount = "1.23456789".parse().expect("valid decimal");

        assert_eq!(format_amount(balance), "1.2345");
    }

    #[test]
    fn writes_the_report_documented_for_the_sample_input() {
        // Guards the example in the README, and exercises every transaction type
        // through the file the CLI contract names.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/transactions.csv");
        let input = std::fs::read_to_string(path).expect("sample input should be readable");

        assert_eq!(
            report(&input),
            "client,available,held,total,locked\n\
             1,1.5000,0.0000,1.5000,false\n\
             2,2.0000,0.0000,2.0000,false\n\
             3,6.5000,0.0000,6.5000,false\n\
             4,0.0000,4.0000,4.0000,false\n\
             5,0.0000,0.0000,0.0000,true\n"
        );
    }
}
