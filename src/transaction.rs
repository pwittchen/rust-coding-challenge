//! Data structures describing the transactions read from the input CSV.

use std::collections::HashMap;

use rust_decimal::Decimal;
use serde::Deserialize;

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
    #[serde(rename = "type")]
    pub kind: TransactionType,
    pub client: ClientId,
    pub tx: TxId,
    pub amount: Option<Amount>,
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
    pub client: ClientId,
    pub amount: Amount,
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
