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
//! - a deposit reusing the ID of an earlier one is ignored (see
//!   [`Engine::deposit`]);
//! - a deposit or a withdrawal opens the account of the client it names, even
//!   when the transaction itself cannot be applied (see [`Engine::withdraw`]);
//! - a dispute may drive the available funds negative (see [`Engine::dispute`]);
//! - an amount carrying more than four decimal places is cut to four (see
//!   [`usable_amount`]).
//!
//! Anything the engine cannot apply — an unknown transaction ID, a resolve for
//! a transaction that is not under dispute, a withdrawal that is not covered —
//! is ignored, as the specification requires, and processing continues.

use std::collections::BTreeMap;

use crate::account::Account;
use crate::transaction::{
    Amount, ClientId, DisputeState, SCALE, Transaction, TransactionRecord, TransactionType,
    Transactions, TxId,
};

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
    #[must_use]
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
    #[must_use]
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
        // Looked up before the account is borrowed, since both live on `self`.
        let repeated = self.transactions.contains_key(&tx);
        let account = self.account(client);

        let Some(amount) = usable_amount(amount) else {
            return;
        };

        if repeated || account.is_locked() {
            return;
        }

        // Recorded only once the money has actually moved, so that a deposit
        // the account refused is not left behind for a dispute to find.
        if account.deposit(amount) {
            self.transactions
                .insert(tx, TransactionRecord::new(client, amount));
        }
    }

    /// Debits the client's account, unless the available funds do not cover the
    /// withdrawal.
    ///
    /// Withdrawals are not recorded, because only deposits can be disputed.
    /// Their ID is therefore free for a later deposit to take, which is harmless
    /// because nothing can refer back to a withdrawal.
    ///
    /// Like a deposit, a withdrawal names the client it is for, so it opens
    /// their account even when it cannot be applied: the client exists as far as
    /// the input is concerned, and an attempt to move money they do not have
    /// leaves them with an empty account rather than with none at all. Disputes,
    /// resolves and chargebacks never open an account, because they only ever
    /// refer back to a transaction whose client already has one.
    fn withdraw(&mut self, client: ClientId, amount: Option<Amount>) {
        let account = self.account(client);

        let Some(amount) = usable_amount(amount) else {
            return;
        };

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
    ///
    /// Freezing therefore strands the funds of any dispute that was still open
    /// when the chargeback landed: nothing can resolve them afterwards, and they
    /// stay held. That is the intended reading of a freeze — the account stops
    /// settling anything until a human looks at it, and money in the middle of a
    /// claim is exactly what should not move on its own in the meantime.
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
///
/// Anything past the fourth decimal place is dropped rather than rounded. The
/// input is specified to carry no more than four, so this only bites on a
/// malformed row, and cutting the excess keeps every balance exactly as precise
/// as the report that prints it — otherwise a fraction too small to show could
/// still be counted, and the reported `available` and `held` would no longer add
/// up to the reported `total`. Dropping it also never credits a client a
/// fraction they did not send.
fn usable_amount(amount: Option<Amount>) -> Option<Amount> {
    amount
        .filter(Amount::is_sign_positive)
        .map(|amount| amount.trunc_with_scale(SCALE))
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

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
            account
                .available()
                .checked_add(account.held())
                .expect("the balances should add up to a representable total"),
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
    fn opens_an_account_for_a_client_whose_transactions_all_fail() {
        let engine = engine(
            "type,client,tx,amount\n\
             withdrawal,3,1,5.0\n\
             deposit,4,2,-1.0\n\
             withdrawal,5,3,\n",
        );

        assert_eq!(engine.accounts().len(), 3);
        for client in [3, 4, 5] {
            assert_balances(&engine, client, "0", "0");
            assert!(!account(&engine, client).is_locked());
        }
    }

    #[test]
    fn opens_no_account_for_a_dispute_referring_to_an_unknown_transaction() {
        let engine = engine(
            "type,client,tx,amount\n\
             dispute,6,1,\n\
             resolve,6,1,\n\
             chargeback,6,1,\n",
        );

        assert_eq!(engine.accounts().len(), 0);
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
    fn cuts_an_amount_carrying_more_than_four_decimal_places() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.00009\n\
             withdrawal,1,2,0.50009\n",
        );

        // Both amounts are cut, not rounded: 1.0000 in, 0.5000 out.
        assert_balances(&engine, 1, "0.5", "0");
    }

    #[test]
    fn holds_the_cut_amount_when_an_over_precise_deposit_is_disputed() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,2.00005\n\
             dispute,1,1,\n",
        );

        // The recorded amount is the one that was credited, so the dispute moves
        // the balance to held in full and leaves nothing behind.
        assert_balances(&engine, 1, "0", "2.0");
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
    fn allows_a_withdrawal_of_the_entire_available_balance() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.2345\n\
             withdrawal,1,2,1.2345\n",
        );

        assert_balances(&engine, 1, "0", "0");
    }

    #[test]
    fn ignores_a_withdrawal_covered_only_by_held_funds() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,5.0\n\
             dispute,1,1,\n\
             withdrawal,1,2,1.0\n",
        );

        assert_balances(&engine, 1, "0", "5.0");
    }

    #[test]
    fn ignores_a_withdrawal_from_an_account_whose_available_funds_are_negative() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             withdrawal,1,2,1.0\n\
             dispute,1,1,\n\
             withdrawal,1,3,0.0001\n",
        );

        assert_balances(&engine, 1, "-1.0", "1.0");
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
    fn records_a_deposit_of_zero_without_changing_the_balance() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,0.0\n\
             deposit,1,1,5.0\n",
        );

        // The second deposit is ignored, which shows the first one took the ID.
        assert_balances(&engine, 1, "0", "0");
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
    fn holds_the_funds_of_several_disputed_deposits_at_once() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,2,2.0\n\
             deposit,1,3,4.0\n\
             dispute,1,1,\n\
             dispute,1,3,\n",
        );

        assert_balances(&engine, 1, "2.0", "5.0");
    }

    #[test]
    fn keeps_the_total_unchanged_through_a_dispute_and_its_resolution() {
        let deposits = "type,client,tx,amount\n\
                        deposit,1,1,3.0\n\
                        deposit,1,2,4.0\n";
        let total = account(&engine(deposits), 1).total();

        let disputed = engine(&format!("{deposits}dispute,1,1,\n"));
        assert_eq!(
            account(&disputed, 1).total(),
            total,
            "a dispute holds funds"
        );

        let resolved = engine(&format!("{deposits}dispute,1,1,\nresolve,1,1,\n"));
        assert_eq!(
            account(&resolved, 1).total(),
            total,
            "a resolve releases them"
        );
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
    fn lets_a_chargeback_drive_the_total_negative() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,5.0\n\
             withdrawal,1,2,5.0\n\
             dispute,1,1,\n\
             chargeback,1,1,\n",
        );

        assert_balances(&engine, 1, "-5.0", "0");
        assert!(account(&engine, 1).is_locked());
    }

    #[test]
    fn keeps_the_funds_of_the_other_disputes_held_when_the_account_is_frozen() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,2,2.0\n\
             dispute,1,1,\n\
             dispute,1,2,\n\
             chargeback,1,1,\n\
             resolve,1,2,\n",
        );

        assert_balances(&engine, 1, "0", "2.0");
        assert!(account(&engine, 1).is_locked());
    }

    #[test]
    fn freezes_only_the_account_of_the_client_that_charged_back() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,2,2,1.0\n\
             dispute,1,1,\n\
             chargeback,1,1,\n\
             deposit,2,3,1.0\n",
        );

        assert_balances(&engine, 1, "0", "0");
        assert!(account(&engine, 1).is_locked());
        assert_balances(&engine, 2, "2.0", "0");
        assert!(!account(&engine, 2).is_locked());
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
    fn ignores_a_dispute_over_a_deposit_that_was_never_applied() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,1,1,5.0\n\
             deposit,1,2,-3.0\n\
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
    fn ignores_a_resolve_raised_by_another_client() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,2,2,1.0\n\
             dispute,1,1,\n\
             resolve,2,1,\n",
        );

        assert_balances(&engine, 1, "0", "1.0");
        assert_balances(&engine, 2, "1.0", "0");
    }

    #[test]
    fn ignores_a_resolve_repeated_after_the_dispute_was_settled() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             dispute,1,1,\n\
             resolve,1,1,\n\
             resolve,1,1,\n",
        );

        assert_balances(&engine, 1, "1.0", "0");
    }

    #[test]
    fn ignores_a_chargeback_raised_by_another_client() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             deposit,2,2,1.0\n\
             dispute,1,1,\n\
             chargeback,2,1,\n",
        );

        assert_balances(&engine, 1, "0", "1.0");
        assert_balances(&engine, 2, "1.0", "0");
        assert!(engine.accounts().all(|account| !account.is_locked()));
    }

    #[test]
    fn ignores_a_chargeback_after_the_dispute_was_resolved() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,1.0\n\
             dispute,1,1,\n\
             resolve,1,1,\n\
             chargeback,1,1,\n",
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
    fn keeps_an_amount_that_no_float_could_hold_exactly() {
        // The balance carries every digit of the deposits, which a float would
        // have rounded away long before the second one was added.
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,1,1,123456789012345.6789\n\
             deposit,1,2,0.0001\n",
        );

        assert_balances(&engine, 1, "123456789012345.6790", "0");
    }

    #[test]
    fn processes_separate_streams_on_separate_threads() {
        // An engine owns everything it touches and shares nothing, so a server
        // can give one to each of the streams it is serving at the same time.
        let inputs = [
            "type,client,tx,amount\ndeposit,1,1,1.0\n",
            "type,client,tx,amount\ndeposit,1,1,2.0\ndispute,1,1,\n",
        ];

        let balances: Vec<_> = std::thread::scope(|scope| {
            let workers: Vec<_> = inputs
                .iter()
                .map(|input| scope.spawn(|| account(&engine(input), 1)))
                .collect();

            workers
                .into_iter()
                .filter_map(|worker| worker.join().ok())
                .collect()
        });

        assert_eq!(balances.len(), 2);
        assert_eq!(balances[0].available(), Decimal::ONE);
        assert_eq!(balances[1].held(), Decimal::TWO);
    }

    #[test]
    fn accepts_the_largest_client_and_transaction_ids() {
        let engine = engine(
            "type,client,tx,amount\n\
             deposit,65535,4294967295,1.0\n\
             dispute,65535,4294967295,\n",
        );

        assert_balances(&engine, ClientId::MAX, "0", "1.0");
    }
}
