//! Reading transactions from the input CSV.

use std::io::Read;
use std::path::Path;

use csv::{ReaderBuilder, Trim};

use crate::transaction::Transaction;

/// How the input is read, for both of the readers below.
///
/// Surrounding whitespace is trimmed, and rows that leave the `amount` field
/// empty — or omit it entirely — are accepted, since disputes, resolves and
/// chargebacks carry no amount. The settings live here, in one place, so that
/// reading a file and reading a stream cannot drift apart.
fn builder() -> ReaderBuilder {
    let mut builder = ReaderBuilder::new();
    builder.trim(Trim::All).flexible(true);

    builder
}

/// Streams the transactions read from `source`.
///
/// Records are decoded lazily, one at a time, so the input never has to be held
/// in memory in full. Any `Read` is accepted, so the same code path serves a
/// file, a socket, or a test fixture.
pub fn read_transactions<R: Read>(source: R) -> impl Iterator<Item = csv::Result<Transaction>> {
    builder().from_reader(source).into_deserialize()
}

/// Streams the transactions from the CSV file at `path`.
///
/// Fails if the file cannot be opened; a malformed record surfaces later, as an
/// error item of the returned iterator.
pub fn read_transactions_from_path(
    path: &Path,
) -> csv::Result<impl Iterator<Item = csv::Result<Transaction>>> {
    Ok(builder().from_path(path)?.into_deserialize())
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use rust_decimal::Decimal;

    use super::*;
    use crate::transaction::TransactionType;

    const SAMPLE: &str = "type, client, tx, amount\n\
                          deposit, 1, 1, 1.0\n\
                          deposit, 2, 2, 2.0\n\
                          deposit, 1, 3, 2.0\n\
                          withdrawal, 1, 4, 1.5\n\
                          withdrawal, 2, 5, 3.0\n";

    fn read_all(input: &str) -> Vec<Transaction> {
        read_transactions(input.as_bytes())
            .collect::<Result<_, _>>()
            .expect("input should be readable")
    }

    #[test]
    fn reads_every_record_in_order() {
        let transactions = read_all(SAMPLE);

        let ids: Vec<_> = transactions.iter().map(|t| t.tx).collect();
        assert_eq!(ids, vec![1, 2, 3, 4, 5]);
        assert_eq!(transactions[3].kind, TransactionType::Withdrawal);
        assert_eq!(transactions[3].client, 1);
        assert_eq!(transactions[3].amount, Some(Decimal::new(15, 1)));
    }

    #[test]
    fn accepts_rows_whose_amount_is_empty_or_missing() {
        let transactions = read_all(
            "type, client, tx, amount\n\
             dispute, 1, 1,\n\
             resolve, 1, 1\n",
        );

        assert_eq!(transactions.len(), 2);
        assert!(transactions.iter().all(|t| t.amount.is_none()));
    }

    #[test]
    fn accepts_headers_and_amounts_without_whitespace() {
        let transactions = read_all("type,client,tx,amount\ndeposit,1,1,1.2345\n");

        assert_eq!(transactions[0].amount, Some(Decimal::new(12345, 4)));
    }

    #[test]
    fn reads_the_columns_in_whatever_order_the_header_gives_them() {
        let transactions = read_all("client, type, amount, tx\n1, deposit, 1.5, 7\n");

        assert_eq!(
            transactions,
            vec![Transaction {
                kind: TransactionType::Deposit,
                client: 1,
                tx: 7,
                amount: Some(Decimal::new(15, 1)),
            }]
        );
    }

    #[test]
    fn reads_an_amount_that_no_float_could_hold_exactly() {
        // Every digit survives the read. A float has around fifteen significant
        // ones, so anything decoded through a float would come back rounded.
        let transactions = read_all("type,client,tx,amount\ndeposit,1,1,123456789012345.6789\n");

        assert_eq!(
            transactions[0].amount,
            Some("123456789012345.6789".parse().expect("valid decimal"))
        );
    }

    #[test]
    fn reads_no_records_from_an_input_without_rows() {
        assert!(read_all("").is_empty());
        assert!(read_all("type, client, tx, amount\n").is_empty());
    }

    #[test]
    fn reports_a_malformed_record_as_an_error_item() {
        // A row the engine cannot make sense of is not skipped: the input itself
        // cannot be trusted, so the run is aborted instead.
        for row in [
            "teleport, 1, 1, 1.0",  // unknown transaction type
            "Deposit, 1, 1, 1.0",   // the type is not spelled in lower case
            "deposit, 65536, 1, 1", // client ID beyond u16
            "deposit, 1, -1, 1.0",  // transaction ID beyond u32
            "deposit, 1, , 1.0",    // no transaction ID
            "deposit, 1, 1, abc",   // amount that is not a decimal
            "deposit, , 1, 1.0",    // no client ID
        ] {
            let input = format!("type, client, tx, amount\n{row}\n");
            let mut records = read_transactions(input.as_bytes());

            assert!(
                records.next().expect("a record is present").is_err(),
                "{row} should not be readable"
            );
        }
    }

    #[test]
    fn reads_transactions_from_a_file() {
        let path = std::env::temp_dir().join("payments-engine-input-test.csv");
        let mut file = std::fs::File::create(&path).expect("temporary file should be creatable");
        file.write_all(SAMPLE.as_bytes())
            .expect("sample should be writable");

        let transactions: Vec<_> = read_transactions_from_path(&path)
            .expect("file should be readable")
            .collect::<Result<_, _>>()
            .expect("records should be decodable");

        assert_eq!(transactions.len(), 5);

        std::fs::remove_file(&path).expect("temporary file should be removable");
    }

    #[test]
    fn fails_when_the_file_does_not_exist() {
        assert!(read_transactions_from_path(Path::new("does-not-exist.csv")).is_err());
    }
}
