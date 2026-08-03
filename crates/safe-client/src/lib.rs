//! Client for the [Safe Transaction Service] v2 API: propose, confirm, and read
//! Safe multisig transactions. Signing uses [`alloy::signers::Signer`] over the
//! EIP-712 `SafeTx` digest, so it works with local keys today and KMS-backed
//! signers later. On-chain execution (`execTransaction`) is out of scope.
//!
//! [Safe Transaction Service]: https://docs.safe.global/core-api/transaction-service-reference/mainnet

mod client;
mod error;
mod middleware;
pub mod model;
mod network;
pub mod signing;

pub use apyx_reqwest_middleware::{HeaderAuthMiddleware, RebaseUrlMiddleware, RouteLabel};
pub use client::SafeClient;
pub use error::Error;
pub use middleware::{API_KEY_HEADER, auth_middleware, rebase_middleware};
pub use network::{DEFAULT_BASE_URL, SafeNetworkSlug, base_url};
pub use signing::{
    MultisigTransactionProposal, Signed, contract_transaction_hash, propose_and_confirm,
};
