//! Data structures describing the transactions read from the input CSV.

use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};

/// Client identifier, as guaranteed by the input format.
pub type ClientId = u16;

/// Globally unique transaction identifier, as guaranteed by the input format.
pub type TxId = u32;

/// Monetary amount.
///
/// A fixed-point decimal is used instead of a float so that four decimal places
/// are represented exactly and repeated additions never accumulate a rounding
/// error, which would be unacceptable for balances.
pub type Amount = Decimal;

/// Number of decimal places an amount is held and reported with, as guaranteed
/// by the input format and required by the output format.
///
/// Amounts are cut to this scale on the way in and rendered at it on the way
/// out, so the balances the engine holds are exactly the ones the report prints.
pub const SCALE: u32 = 4;

/// The kind of a transaction, taken from the `type` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    /// Credit to the client's account: increases available and total funds.
    Deposit,
    /// Debit from the client's account: decreases available and total funds.
    Withdrawal,
    /// Claim that a referenced transaction was erroneous; moves funds to held.
    Dispute,
    /// Resolution of a dispute; moves held funds back to available.
    Resolve,
    /// Reversal of a disputed transaction; removes held funds and locks the account.
    Chargeback,
}

/// A single row of the input CSV.
///
/// `amount` is optional because disputes, resolves and chargebacks reference a
/// transaction by ID and carry no amount of their own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Transaction {
    /// What the transaction does, from the `type` column.
    #[serde(rename = "type")]
    pub kind: TransactionType,
    /// The client whose account the transaction is about.
    pub client: ClientId,
    /// The transaction's own ID, or — for a dispute, resolve or chargeback —
    /// the ID of the transaction it refers back to.
    pub tx: TxId,
    /// The amount of money to move, for the kinds that move any.
    #[serde(deserialize_with = "deserialize_amount")]
    pub amount: Option<Amount>,
}

/// Reads an amount from the text of the field, and from nothing else.
///
/// Without this, the field is decoded by asking the CSV reader what the value
/// looks like, and a value that looks like a number is handed over as an `f64` —
/// which is exactly the conversion the fixed-point type exists to avoid, and
/// would silently round an amount such as `123456789012345.6789` on the way in.
/// Reading the digits directly keeps every amount exact, and is also markedly
/// faster than having the reader guess the type of every field first.
///
/// An empty field, or a row that stops before this column, is not an amount but
/// the absence of one: disputes, resolves and chargebacks carry no amount.
fn deserialize_amount<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Amount>, D::Error> {
    struct AmountVisitor;

    impl<'de> Visitor<'de> for AmountVisitor {
        type Value = Option<Amount>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a decimal amount, or nothing")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
            let value = value.trim();
            if value.is_empty() {
                return Ok(None);
            }

            Decimal::from_str(value).map(Some).map_err(E::custom)
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_str(self)
        }
    }

    deserializer.deserialize_option(AmountVisitor)
}

/// Where a recorded transaction stands in the dispute lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisputeState {
    /// No dispute has been opened, or an earlier one was resolved.
    #[default]
    Undisputed,
    /// Funds are currently held pending the outcome of a dispute.
    Disputed,
    /// The dispute ended in a chargeback; the transaction is final.
    ChargedBack,
}

/// A transaction retained after processing so that a later dispute, resolve or
/// chargeback can refer back to it by ID.
///
/// Only the fields needed to settle a dispute are kept, so memory grows with the
/// number of referable transactions rather than with the size of the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionRecord {
    /// The client the transaction belonged to, so that a dispute raised by
    /// anybody else can be turned away.
    pub client: ClientId,
    /// The amount that was actually applied, which is what a dispute holds and
    /// a chargeback reverses.
    pub amount: Amount,
    /// Where the transaction stands in the dispute lifecycle.
    pub state: DisputeState,
}

impl TransactionRecord {
    /// Records a newly processed transaction that is not yet disputed.
    pub fn new(client: ClientId, amount: Amount) -> Self {
        Self {
            client,
            amount,
            state: DisputeState::Undisputed,
        }
    }
}

/// The transactions seen so far, keyed by their globally unique ID.
///
/// A map rather than a list, because disputes, resolves and chargebacks always
/// look a transaction up by ID and never iterate over the history.
///
/// The standard library's hasher is kept deliberately, rather than swapped for
/// one of the faster ones that suit a `u32` key. Transaction IDs are chosen by
/// whoever sends the input, so a faster hasher would let a hostile partner pick
/// IDs that all land in one bucket and turn every lookup into a linear scan.
/// Paying for a hash that cannot be gamed is the right trade for an engine meant
/// to survive a stream it does not control.
pub type Transactions = HashMap<TxId, TransactionRecord>;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Vec<Transaction> {
        csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(input.as_bytes())
            .deserialize()
            .collect::<Result<_, _>>()
            .expect("input should deserialize")
    }

    #[test]
    fn parses_a_transaction_carrying_an_amount() {
        let transactions = parse("type, client, tx, amount\ndeposit, 1, 3, 2.0001\n");

        assert_eq!(
            transactions,
            vec![Transaction {
                kind: TransactionType::Deposit,
                client: 1,
                tx: 3,
                amount: Some(Decimal::new(20001, 4)),
            }]
        );
    }

    #[test]
    fn parses_a_transaction_without_an_amount() {
        let transactions = parse("type, client, tx, amount\ndispute, 1, 3,\n");

        assert_eq!(
            transactions,
            vec![Transaction {
                kind: TransactionType::Dispute,
                client: 1,
                tx: 3,
                amount: None,
            }]
        );
    }

    #[test]
    fn parses_every_transaction_type() {
        let transactions = parse(
            "type, client, tx, amount\n\
             deposit, 1, 1, 1.0\n\
             withdrawal, 1, 2, 1.0\n\
             dispute, 1, 1,\n\
             resolve, 1, 1,\n\
             chargeback, 1, 1,\n",
        );

        let kinds: Vec<_> = transactions.iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TransactionType::Deposit,
                TransactionType::Withdrawal,
                TransactionType::Dispute,
                TransactionType::Resolve,
                TransactionType::Chargeback,
            ]
        );
    }
}
