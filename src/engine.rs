//! Applying transactions to client accounts.
//!
//! The engine consumes transactions one at a time, in the order they appear in
//! the input, and keeps the resulting state of every client account.
//!
//! # Interpretation of the specification
//!
//! The specification leaves a few cases open. They are resolved the way a bank
//! would resolve them, and the reasoning is documented on the method that
//! implements each decision:
//!
//! - only deposits can be disputed (see [`Engine::dispute`]);
//! - a dispute, resolve or chargeback must come from the client that owns the
//!   referenced transaction;
//! - a frozen account accepts no further transactions of any kind;
//! - a transaction ID is used at most once (see [`Engine::deposit`]);
//! - a dispute may drive the available funds negative (see [`Engine::dispute`]).
//!
//! Anything the engine cannot apply — an unknown transaction ID, a resolve for
//! a transaction that is not under dispute, a withdrawal that is not covered —
//! is ignored, as the specification requires, and processing continues.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use crate::transaction::{
    Amount, ClientId, DisputeState, Transaction, TransactionRecord, TransactionType, Transactions,
    TxId,
};

/// The state of a single client's asset account.
///
/// The balances are private so that they can only change through the operations
/// below, each of which upholds the invariant that `total = available + held`
/// and that all three amounts stay representable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    client: ClientId,
    available: Amount,
    held: Amount,
    locked: bool,
}

impl Account {
    /// Opens an empty account for `client`.
    pub fn new(client: ClientId) -> Self {
        Self {
            client,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    /// The account's owner.
    pub fn client(&self) -> ClientId {
        self.client
    }

    /// Funds available for trading, staking, withdrawal, etc.
    pub fn available(&self) -> Amount {
        self.available
    }

    /// Funds held pending the outcome of a dispute.
    pub fn held(&self) -> Amount {
        self.held
    }

    /// Funds that are either available or held.
    pub fn total(&self) -> Amount {
        // Cannot overflow: every mutation rejects a change whose resulting
        // total is not representable.
        self.available + self.held
    }

    /// Whether the account is frozen, which happens on a chargeback.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Credits `amount` to the available funds.
    fn deposit(&mut self, amount: Amount) -> bool {
        self.shift(amount, Decimal::ZERO)
    }

    /// Debits `amount` from the available funds, unless they do not cover it.
    fn withdraw(&mut self, amount: Amount) -> bool {
        self.available >= amount && self.shift(-amount, Decimal::ZERO)
    }

    /// Moves `amount` from the available funds to the held funds.
    fn hold(&mut self, amount: Amount) -> bool {
        self.shift(-amount, amount)
    }

    /// Moves `amount` from the held funds back to the available funds.
    fn release(&mut self, amount: Amount) -> bool {
        self.shift(amount, -amount)
    }

    /// Withdraws `amount` from the held funds and freezes the account.
    fn reverse(&mut self, amount: Amount) -> bool {
        let reversed = self.shift(Decimal::ZERO, -amount);
        if reversed {
            self.locked = true;
        }

        reversed
    }

    /// Adds the two deltas to the respective balances.
    ///
    /// The account is left untouched, and `false` returned, if any of the
    /// resulting amounts would overflow. Balances are therefore never left
    /// half-updated, and no arithmetic on them can panic later.
    fn shift(&mut self, available: Amount, held: Amount) -> bool {
        let (Some(available), Some(held)) = (
            self.available.checked_add(available),
            self.held.checked_add(held),
        ) else {
            return false;
        };

        if available.checked_add(held).is_none() {
            return false;
        }

        self.available = available;
        self.held = held;

        true
    }
}

/// A payments engine: it applies transactions to the accounts of its clients.
///
/// Accounts are kept in a map keyed by client ID, so memory grows with the
/// number of clients (at most `u16::MAX`) and with the number of transactions
/// that can still be disputed — never with the length of the input, which is
/// consumed as a stream.
#[derive(Debug, Default)]
pub struct Engine {
    /// Ordered by client ID, which makes the report deterministic. Row order
    /// does not matter to the output format, but it does make the program
    /// easier to test and to diff.
    accounts: BTreeMap<ClientId, Account>,
    /// The deposits seen so far, which are the transactions a dispute can refer
    /// back to.
    transactions: Transactions,
}

impl Engine {
    /// Creates an engine with no clients and no history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `transaction`, ignoring it if it cannot be applied.
    pub fn apply(&mut self, transaction: &Transaction) {
        let Transaction {
            kind,
            client,
            tx,
            amount,
        } = *transaction;

        match kind {
            TransactionType::Deposit => self.deposit(client, tx, amount),
            TransactionType::Withdrawal => self.withdraw(client, amount),
            TransactionType::Dispute => self.dispute(client, tx),
            TransactionType::Resolve => self.resolve(client, tx),
            TransactionType::Chargeback => self.chargeback(client, tx),
        }
    }

    /// The state of every account the engine has seen, ordered by client ID.
    pub fn accounts(&self) -> impl ExactSizeIterator<Item = &Account> {
        self.accounts.values()
    }

    /// Credits the client's account and records the deposit, so that it can be
    /// disputed later.
    ///
    /// A deposit whose ID has already been used is ignored: transaction IDs are
    /// globally unique, so a repeated one is an error on our partner's side, and
    /// honouring it would make a later dispute ambiguous.
    fn deposit(&mut self, client: ClientId, tx: TxId, amount: Option<Amount>) {
        let Some(amount) = usable_amount(amount) else {
            return;
        };

        if self.transactions.contains_key(&tx) {
            return;
        }

        let account = self.account(client);
        if account.is_locked() || !account.deposit(amount) {
            return;
        }

        self.transactions
            .insert(tx, TransactionRecord::new(client, amount));
    }

    /// Debits the client's account, unless the available funds do not cover the
    /// withdrawal.
    ///
    /// Withdrawals are not recorded, because only deposits can be disputed.
    fn withdraw(&mut self, client: ClientId, amount: Option<Amount>) {
        let Some(amount) = usable_amount(amount) else {
            return;
        };

        let account = self.account(client);
        if account.is_locked() {
            return;
        }

        account.withdraw(amount);
    }

    /// Holds the funds of the disputed deposit.
    ///
    /// Only deposits are disputable. The specification defines a dispute as
    /// moving funds from available to held, which only makes sense for money
    /// that was paid in: holding the amount of a withdrawal would take funds the
    /// client never received. A dispute over a withdrawal is therefore treated
    /// like a dispute over an unknown transaction and ignored.
    ///
    /// The available funds may go negative, when the deposit under dispute has
    /// already been spent. That is deliberate: the total must not change, so the
    /// held amount has to come out of the available funds whether they cover it
    /// or not, leaving the client owing the difference — exactly what a bank
    /// does with a claimed-back payment.
    fn dispute(&mut self, client: ClientId, tx: TxId) {
        let Some((account, record)) = self.disputable(client, tx) else {
            return;
        };

        if record.state == DisputeState::Undisputed && account.hold(record.amount) {
            record.state = DisputeState::Disputed;
        }
    }

    /// Releases the funds held for a dispute that ended without a chargeback.
    fn resolve(&mut self, client: ClientId, tx: TxId) {
        let Some((account, record)) = self.disputable(client, tx) else {
            return;
        };

        if record.state == DisputeState::Disputed && account.release(record.amount) {
            record.state = DisputeState::Undisputed;
        }
    }

    /// Reverses the disputed deposit and freezes the account.
    fn chargeback(&mut self, client: ClientId, tx: TxId) {
        let Some((account, record)) = self.disputable(client, tx) else {
            return;
        };

        if record.state == DisputeState::Disputed && account.reverse(record.amount) {
            record.state = DisputeState::ChargedBack;
        }
    }

    /// The account and the recorded deposit a dispute, resolve or chargeback
    /// refers to, if the reference can be honoured at all.
    ///
    /// It cannot when the transaction is unknown, when it belongs to another
    /// client — a client may only dispute their own transactions — or when the
    /// account is frozen.
    fn disputable(
        &mut self,
        client: ClientId,
        tx: TxId,
    ) -> Option<(&mut Account, &mut TransactionRecord)> {
        let record = self.transactions.get_mut(&tx)?;
        if record.client != client {
            return None;
        }

        // The account exists, because it was created by the recorded deposit.
        let account = self.accounts.get_mut(&client)?;
        if account.is_locked() {
            return None;
        }

        Some((account, record))
    }

    /// The client's account, opening an empty one if this is their first
    /// transaction.
    fn account(&mut self, client: ClientId) -> &mut Account {
        self.accounts
            .entry(client)
            .or_insert_with(|| Account::new(client))
    }
}

/// The amount to credit or debit, if the row carries one that a bank would act
/// on.
///
/// A missing amount, or a negative one, is an error on our partner's side: a
/// negative deposit is a withdrawal in disguise, and would bypass the check that
/// available funds cover it.
fn usable_amount(amount: Option<Amount>) -> Option<Amount> {
    amount.filter(|amount| amount.is_sign_positive())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::read_transactions;

    /// Builds an engine from a CSV, exercising the same path as the binary.
    fn engine(input: &str) -> Engine {
        let mut engine = Engine::new();
        for transaction in read_transactions(input.as_bytes()) {
            engine.apply(&transaction.expect("input should be readable"));
        }

        engine
    }

    fn account(engine: &Engine, client: ClientId) -> Account {
        engine
            .accounts()
            .find(|account| account.client() == client)
            .cloned()
            .expect("client should have an account")
    }

    /// Asserts the balances of `client`, given as decimal literals.
    fn assert_balances(engine: &Engine, client: ClientId, available: &str, held: &str) {
        let account = account(engine, client);
        let expected = |value: &str| value.parse::<Decimal>().expect("valid decimal");

        assert_eq!(account.available(), expected(available), "available funds");
        assert_eq!(account.held(), expected(held), "held funds");
        assert_eq!(
            account.total(),
            account.available() + account.held(),
            "total funds"
        );
    }

    #[test]
    fn applies_the_example_from_the_specification() {
        let engine = engine(
            "type, client, tx, amount\n\
             deposit, 1, 1, 1.0\n\
             deposit, 2, 2, 2.0\n\
             deposit, 1, 3, 2.0\n\
             withdrawal, 1, 4, 1.5\n\
             withdrawal, 2, 5, 3.0\n",
        );

        assert_eq!(engine.accounts().len(), 2);
        assert_balances(&engine, 1, "1.5", "0");
        assert_balances(&engine, 2, "2.0", "0");
        assert!(engine.accounts().all(|account| !account.is_locked()));
    }

    #[test]
    fn opens_an_account_for_an_unknown_client() {
        let engine = engine("type,client,tx,amount\ndeposit,7,1,1.0\n");

        assert_balances(&engine, 7, "1.0", "0");
    }

    #[test]
    fn keeps_the_full_precision_of_four_decimal_places() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0001\n\
             deposit,1,2,0.0002\n\
             withdrawal,1,3,0.0003\n",
        );

        assert_balances(&engine, 1, "1.0000", "0");
    }

    #[test]
    fn ignores_a_withdrawal_that_available_funds_do_not_cover() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             withdrawal,1,2,1.0001\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn ignores_a_repeated_transaction_id() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,1,5.0\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn ignores_deposits_and_withdrawals_without_a_usable_amount() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,2,\n\
             deposit,1,3,-5.0\n\
             withdrawal,1,4,-1.0\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn holds_the_funds_of_a_disputed_deposit() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,2,2.0\n\
             dispute,1,1,\n",
        );

        assert_balances(&engine, 1, "2.0", "1.0");
        assert!(!account(&engine, 1).is_locked());
    }

    #[test]
    fn lets_a_dispute_drive_the_available_funds_negative() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             withdrawal,1,2,1.0\n\
             dispute,1,1,\n",
        );

        assert_balances(&engine, 1, "-1.0", "1.0");
    }

    #[test]
    fn releases_the_held_funds_on_a_resolve() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             dispute,1,1,\n\
             resolve,1,1,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
        assert!(!account(&engine, 1).is_locked());
    }

    #[test]
    fn reverses_the_transaction_and_freezes_the_account_on_a_chargeback() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,2,2.0\n\
             dispute,1,1,\n\
             chargeback,1,1,\n",
        );

        assert_balances(&engine, 1, "2.0", "0");
        assert!(account(&engine, 1).is_locked());
    }

    #[test]
    fn ignores_transactions_on_a_frozen_account() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,2,2.0\n\
             dispute,1,1,\n\
             chargeback,1,1,\n\
             deposit,1,3,10.0\n\
             withdrawal,1,4,1.0\n\
             dispute,1,2,\n",
        );

        assert_balances(&engine, 1, "2.0", "0");
        assert!(account(&engine, 1).is_locked());
    }

    #[test]
    fn ignores_a_dispute_referring_to_an_unknown_transaction() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             dispute,1,404,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn ignores_a_dispute_over_a_withdrawal() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,2.0\n\
             withdrawal,1,2,1.0\n\
             dispute,1,2,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn ignores_a_dispute_raised_by_another_client() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,2,2,1.0\n\
             dispute,2,1,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
        assert_balances(&engine, 2, "1.0", "0");
    }

    #[test]
    fn ignores_a_dispute_over_an_already_disputed_transaction() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             dispute,1,1,\n\
             dispute,1,1,\n",
        );

        assert_balances(&engine, 1, "0", "1.0");
    }

    #[test]
    fn ignores_a_resolve_for_a_transaction_that_is_not_under_dispute() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             resolve,1,1,\n\
             resolve,1,404,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn ignores_a_chargeback_for_a_transaction_that_is_not_under_dispute() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             chargeback,1,1,\n\
             chargeback,1,404,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
        assert!(!account(&engine, 1).is_locked());
    }

    #[test]
    fn allows_a_resolved_transaction_to_be_disputed_again() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             dispute,1,1,\n\
             resolve,1,1,\n\
             dispute,1,1,\n",
        );

        assert_balances(&engine, 1, "0", "1.0");
    }

    #[test]
    fn keeps_the_accounts_of_different_clients_apart() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,2,1,5.0\n\
             deposit,1,2,1.0\n\
             withdrawal,2,3,2.0\n\
             dispute,1,2,\n",
        );

        let clients: Vec<_> = engine.accounts().map(|account| account.client()).collect();
        assert_eq!(clients, vec![1, 2], "accounts are ordered by client ID");
        assert_balances(&engine, 1, "0", "1.0");
        assert_balances(&engine, 2, "3.0", "0");
    }

    #[test]
    fn leaves_the_account_untouched_when_a_deposit_would_overflow() {
        let mut account = Account::new(1);
        assert!(account.deposit(Decimal::MAX));
        assert!(!account.deposit(Decimal::MAX));
        assert_eq!(account.available(), Decimal::MAX);
        assert_eq!(account.total(), Decimal::MAX);
    }

    #[test]
    fn leaves_the_account_untouched_when_the_total_would_overflow() {
        let mut account = Account::new(1);
        assert!(account.deposit(Decimal::MAX));
        assert!(account.hold(Decimal::MAX));

        assert!(!account.deposit(Decimal::ONE));
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::MAX);
        assert_eq!(account.total(), Decimal::MAX);
    }
}
