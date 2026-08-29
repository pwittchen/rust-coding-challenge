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
/// trailing zeros, so that every balance is reported with the same four decimal
/// places no matter how it was arrived at. Nothing is lost in the process: the
/// engine already holds every balance at this scale.
///
/// A balance that lands on exactly zero is reported as a positive zero. The
/// decimal keeps a sign of its own, so subtracting a zero amount — holding the
/// funds of a deposit too small to register, say — leaves a negative zero
/// behind, and `-0.0000` is not a balance any reader of the report expects.
fn format_amount(value: Amount) -> String {
    let value = if value.is_zero() { Amount::ZERO } else { value };

    format!("{:.*}", SCALE as usize, value)
}

/// Writes `accounts` to `writer` as CSV, one row per client.
///
/// The header is always written, so the output is a well-formed CSV even when
/// there are no accounts to report.
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
