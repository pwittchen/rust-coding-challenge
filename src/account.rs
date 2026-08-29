//! The state of a client's account and the operations that change it.
//!
//! The account owns its balances and is the only place where they move. Every
//! operation is checked, so the invariants below hold whatever the engine
//! applies to it.

use rust_decimal::Decimal;

use crate::transaction::{Amount, ClientId};

/// The state of a single client's asset account.
///
/// The balances are private so that they can only change through the operations
/// below, each of which upholds the invariant that `total = available + held`
/// and that all three amounts stay representable.
///
/// Every operation that moves money returns whether it did: `true` when the
/// balances changed, `false` when the operation was refused and the account was
/// left exactly as it was. There is no third outcome — an operation never
/// half-applies — so the caller can act on the answer as a plain yes or no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    client: ClientId,
    available: Amount,
    held: Amount,
    locked: bool,
}

impl Account {
    /// Opens an empty account for `client`.
    #[must_use]
    pub fn new(client: ClientId) -> Self {
        Self {
            client,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
        }
    }

    /// The account's owner.
    #[must_use]
    pub fn client(&self) -> ClientId {
        self.client
    }

    /// Funds available for trading, staking, withdrawal, etc.
    #[must_use]
    pub fn available(&self) -> Amount {
        self.available
    }

    /// Funds held pending the outcome of a dispute.
    #[must_use]
    pub fn held(&self) -> Amount {
        self.held
    }

    /// Funds that are either available or held.
    #[must_use]
    pub fn total(&self) -> Amount {
        // The saturation is unreachable: every mutation rejects a change whose
        // resulting total is not representable, so the sum always fits. It is
        // written as a checked operation regardless, so that the guarantee is
        // the compiler's rather than this comment's — a plain `+` on a decimal
        // panics on overflow, which is the one thing this crate must never do.
        self.available.saturating_add(self.held)
    }

    /// Whether the account is frozen, which happens on a chargeback.
    #[must_use]
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Credits `amount` to the available funds.
    pub(crate) fn deposit(&mut self, amount: Amount) -> bool {
        self.shift(amount, Decimal::ZERO)
    }

    /// Debits `amount` from the available funds, unless they do not cover it.
    ///
    /// The engine has nothing to do when a withdrawal is refused — the
    /// specification says to ignore it — so this is the one operation whose
    /// answer the caller may drop.
    pub(crate) fn withdraw(&mut self, amount: Amount) -> bool {
        self.available >= amount && self.shift(-amount, Decimal::ZERO)
    }

    /// Moves `amount` from the available funds to the held funds.
    pub(crate) fn hold(&mut self, amount: Amount) -> bool {
        self.shift(-amount, amount)
    }

    /// Moves `amount` from the held funds back to the available funds.
    pub(crate) fn release(&mut self, amount: Amount) -> bool {
        self.shift(amount, -amount)
    }

    /// Withdraws `amount` from the held funds and freezes the account.
    pub(crate) fn reverse(&mut self, amount: Amount) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_an_empty_account_that_is_not_frozen() {
        let account = Account::new(7);

        assert_eq!(account.client(), 7);
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::ZERO);
        assert_eq!(account.total(), Decimal::ZERO);
        assert!(!account.is_locked());
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

    #[test]
    fn leaves_the_account_untouched_when_a_dispute_would_overflow() {
        let mut account = Account::new(1);
        assert!(account.deposit(Decimal::MAX));
        assert!(account.hold(Decimal::MAX));

        assert!(!account.hold(Decimal::MAX));
        assert_eq!(account.available(), Decimal::ZERO);
        assert_eq!(account.held(), Decimal::MAX);
    }
}
