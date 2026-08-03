//! SafeTx EIP-712 hashing and signed-proposal state.

mod safe_tx;
mod state;

pub use safe_tx::{SafeTxFields, contract_transaction_hash};
pub use state::{Signed, propose_and_confirm};

pub use crate::model::MultisigTransactionProposal;
