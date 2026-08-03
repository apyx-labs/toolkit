use alloy::{
    primitives::{B256, Bytes, Signature},
    signers::Signer,
};

use crate::{
    Error,
    model::{CreateMultisigTransactionRequest, MultisigTransactionProposal},
    signing::contract_transaction_hash,
};

/// A multisig proposal that has been signed by one owner. Ready to be turned
/// into a [`CreateMultisigTransactionRequest`] body.
#[derive(Debug, Clone)]
pub struct Signed {
    pub inner: MultisigTransactionProposal,
    pub contract_transaction_hash: B256,
    pub sender: alloy::primitives::Address,
    pub signature: Signature,
}

impl MultisigTransactionProposal {
    /// Signs this proposal's SafeTx EIP-712 digest with `signer`. Uses
    /// `sign_hash` (the EIP-712 prehash), not `sign_message` (EIP-191).
    pub async fn sign<S: Signer + Sync>(
        self,
        signer: &S,
        safe: alloy::primitives::Address,
        chain_id: u64,
    ) -> Result<Signed, Error> {
        let hash = contract_transaction_hash(&self.to_safe_tx_fields(), safe, chain_id);
        let signature = signer.sign_hash(&hash).await.map_err(Error::Signing)?;
        Ok(Signed {
            inner: self,
            contract_transaction_hash: hash,
            sender: signer.address(),
            signature,
        })
    }
}

impl Signed {
    /// Builds the create-transaction request body for the given Safe address.
    /// Carries the per-owner signature (NOT the concatenated on-chain
    /// `execTransaction` encoding).
    pub fn to_create_request(
        &self,
        safe: alloy::primitives::Address,
    ) -> CreateMultisigTransactionRequest {
        let p = &self.inner;
        CreateMultisigTransactionRequest {
            safe,
            to: p.to,
            value: p.value,
            data: (!p.data.is_empty()).then(|| p.data.clone()),
            operation: p.operation as u8,
            gas_token: p.gas_token,
            safe_tx_gas: p.safe_tx_gas,
            base_gas: p.base_gas,
            gas_price: p.gas_price,
            refund_receiver: p.refund_receiver,
            nonce: p.nonce,
            contract_transaction_hash: self.contract_transaction_hash,
            sender: self.sender,
            signature: self.signature_bytes(),
            origin: p.origin.clone(),
        }
    }

    /// The raw 65-byte ECDSA signature used for both the create body and
    /// `POST .../confirmations/`. Serializes as a `0x`-prefixed hex string.
    pub fn signature_bytes(&self) -> Bytes {
        Bytes::copy_from_slice(&self.signature.as_bytes())
    }
}

/// Convenience: sign + propose a Safe transaction and return the resulting
/// transaction record.
///
/// This is propose-with-confirmation in the Safe sense — the proposer's
/// signature is included in the create body and the Safe Transaction
/// Service registers it as the first owner confirmation **atomically** as
/// part of the create call. The `signature` field in
/// [`CreateMultisigTransactionRequest`] *is* that first confirmation; no
/// separate `POST /multisig-transactions/<hash>/confirmations/` is needed.
///
/// We must NOT also call the `/confirmations/` endpoint for the proposer:
/// it is reserved for *other* owners adding their signatures to an
/// already-proposed tx. Calling it for the proposer is redundant and, in
/// practice, races the service's read-replica indexing of the just-created
/// tx — yielding spurious `HTTP 404 Not Found` responses when the
/// lookup-by-hash arrives before the index catches up.
///
/// For multi-owner Safes, the caller is responsible for collecting further
/// confirmations from the remaining owners (e.g. a Blockaid cosigner) via
/// [`SafeClient::safe_confirm_multisig_transaction`].
pub async fn propose_and_confirm<C, S>(
    client: &C,
    safe: alloy::primitives::Address,
    chain_id: u64,
    proposal: MultisigTransactionProposal,
    signer: &S,
) -> Result<crate::model::MultisigTransaction, Error>
where
    C: crate::client::SafeClient,
    S: Signer + Sync,
{
    let signed = proposal.sign(signer, safe, chain_id).await?;
    client
        .safe_create_multisig_transaction(safe, &signed.to_create_request(safe))
        .await?;
    client
        .safe_get_multisig_transaction(signed.contract_transaction_hash)
        .await
}

#[cfg(test)]
mod tests {
    use alloy::{
        primitives::{Address, B256, Bytes, U256},
        signers::local::PrivateKeySigner,
    };

    use super::*;
    use crate::{model::Operation, signing::contract_transaction_hash};

    fn signer() -> PrivateKeySigner {
        PrivateKeySigner::from_bytes(&B256::from([0x11u8; 32])).expect("valid key")
    }

    fn proposal() -> MultisigTransactionProposal {
        MultisigTransactionProposal {
            to: Address::with_last_byte(0x22),
            value: U256::from(1u64),
            data: Bytes::new(),
            operation: Operation::Call,
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            gas_token: None,
            refund_receiver: None,
            nonce: U256::from(3u64),
            origin: Some("apyx".to_string()),
        }
    }

    #[tokio::test]
    async fn signature_recovers_to_signer() {
        let signer = signer();
        let safe = Address::with_last_byte(0xaa);
        let chain_id = 1;
        let proposal = proposal();

        let digest = contract_transaction_hash(&proposal.to_safe_tx_fields(), safe, chain_id);
        let signed = proposal.sign(&signer, safe, chain_id).await.expect("sign");

        assert_eq!(signed.contract_transaction_hash, digest);
        let recovered = signed
            .signature
            .recover_address_from_prehash(&digest)
            .expect("recover");
        assert_eq!(recovered, signer.address());
    }

    #[tokio::test]
    async fn signed_serializes_into_create_body() {
        let signer = signer();
        let safe = Address::with_last_byte(0xaa);
        let signed = proposal().sign(&signer, safe, 1).await.expect("sign");

        let body = signed.to_create_request(safe);
        let v = serde_json::to_value(&body).expect("serialize");
        // Safe Transaction Service v2 requires EIP-55 checksum-cased addresses.
        assert_eq!(v["safe"], serde_json::Value::String(safe.to_checksum(None)));
        assert_eq!(
            v["sender"],
            serde_json::Value::String(signer.address().to_checksum(None))
        );
        assert_eq!(v["operation"], 0);
        assert_eq!(v["nonce"], "3");
        assert!(v["signature"].as_str().unwrap().starts_with("0x"));
        assert_eq!(
            v["contractTransactionHash"],
            serde_json::to_value(signed.contract_transaction_hash).unwrap()
        );
        // 65-byte ECDSA => 132 hex chars + "0x".
        assert_eq!(v["signature"].as_str().unwrap().len(), 2 + 130);
    }

    /// Regression: the Safe Transaction Service v2 returns HTTP 422
    /// ("Address X is not checksumed") for any all-lowercase address with at
    /// least one mixed-case nibble. Test against real-world mainnet addresses
    /// (MultiSend, a Safe owner) whose checksum case differs from lowercase.
    #[tokio::test]
    async fn mixed_case_address_is_emitted_in_eip55_checksum() {
        use core::str::FromStr;

        let signer = signer();
        // Real MultiSend address (mainnet, used by morpho-liquidator).
        let to = Address::from_str("0x9641D764FC13C8B624c04430c7356C1c7c8102e2").unwrap();
        let safe = Address::from_str("0x2e8b37B64467E9Dc627a2Db9aE7108255eFf1Efb").unwrap();

        let mut proposal = proposal();
        proposal.to = to;
        proposal.gas_token = Some(to);
        proposal.refund_receiver = Some(safe);

        let signed = proposal.sign(&signer, safe, 1).await.expect("sign");
        let body = signed.to_create_request(safe);
        let v = serde_json::to_value(&body).expect("serialize");

        // Each address is emitted in EIP-55 checksum case (alloy's canonical form).
        assert_eq!(v["safe"], serde_json::Value::String(safe.to_checksum(None)));
        assert_eq!(v["to"], serde_json::Value::String(to.to_checksum(None)));
        assert_eq!(
            v["gasToken"],
            serde_json::Value::String(to.to_checksum(None))
        );
        assert_eq!(
            v["refundReceiver"],
            serde_json::Value::String(safe.to_checksum(None))
        );
        assert_eq!(
            v["sender"],
            serde_json::Value::String(signer.address().to_checksum(None))
        );
        // Belt-and-suspenders: confirm the emitted form is NOT all-lowercase,
        // which is what would have been rejected by the live service.
        assert_ne!(
            v["to"].as_str().unwrap(),
            format!("{:#x}", to),
            "address must be EIP-55 checksum, not lowercase"
        );
    }
}
