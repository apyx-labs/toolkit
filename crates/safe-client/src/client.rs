//! The `SafeClient` extension trait.
//!
//! With the `reqwest` feature, [`reqwest`] provides [`SafeClient`] for
//! [`reqwest_middleware::ClientWithMiddleware`].

use alloy::primitives::{Address, B256, Bytes};

use crate::{
    Error,
    model::{ConfirmationList, CreateMultisigTransactionRequest, MultisigTransaction},
};

#[cfg(feature = "reqwest")]
pub mod reqwest;

/// Operations against the [Safe Transaction Service] v2 API.
///
/// With the `reqwest` feature, implemented for
/// [`reqwest_middleware::ClientWithMiddleware`] in [`reqwest`]. The target
/// network is pinned at construction by registering
/// [`rebase_middleware`](crate::rebase_middleware); auth is a
/// [`HeaderAuthMiddleware`](crate::HeaderAuthMiddleware). Every request
/// carries a static [`RouteLabel`](crate::RouteLabel) for low-cardinality
/// metrics.
///
/// [Safe Transaction Service]: https://docs.safe.global/core-api/transaction-service-reference/mainnet
#[allow(async_fn_in_trait)]
pub trait SafeClient {
    /// `POST /safes/{address}/multisig-transactions/` — proposes a transaction.
    /// The request body carries the proposer's signature, which the Safe
    /// Transaction Service registers atomically as the first owner
    /// confirmation. Prefer the high-level helper [`propose_and_confirm`](crate::propose_and_confirm)
    /// which signs the EIP-712 digest and issues this call in one step.
    async fn safe_create_multisig_transaction(
        &self,
        safe: Address,
        request: &CreateMultisigTransactionRequest,
    ) -> Result<(), Error>;

    /// `GET /multisig-transactions/{safe_tx_hash}/` — fetches a transaction.
    async fn safe_get_multisig_transaction(
        &self,
        safe_tx_hash: B256,
    ) -> Result<MultisigTransaction, Error>;

    /// `GET /multisig-transactions/{safe_tx_hash}/confirmations/` — lists the
    /// owner confirmations collected so far.
    async fn safe_list_multisig_confirmations(
        &self,
        safe_tx_hash: B256,
    ) -> Result<ConfirmationList, Error>;

    /// `POST /multisig-transactions/{safe_tx_hash}/confirmations/` — appends an
    /// owner's signature. `signature` is the raw signature bytes (typically a
    /// 65-byte ECDSA signature), serialized on the wire as `0x`-prefixed hex.
    ///
    /// Reserved for *other* owners after a proposal exists; the proposer's
    /// signature is already attached to the transaction by
    /// [`safe_create_multisig_transaction`](Self::safe_create_multisig_transaction).
    async fn safe_confirm_multisig_transaction(
        &self,
        safe_tx_hash: B256,
        signature: Bytes,
    ) -> Result<(), Error>;
}
