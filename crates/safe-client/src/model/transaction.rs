use alloy::primitives::{Address, B256, Bytes, U256};
use serde::{Deserialize, Serialize};

use crate::{model::Confirmation, signing::SafeTxFields};

/// Safe call operation type. The discriminants are the on-wire `operation`
/// integers, so `op as u8` yields the value the API expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Operation {
    Call = 0,
    DelegateCall = 1,
}

impl TryFrom<u8> for Operation {
    type Error = String;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Operation::Call),
            1 => Ok(Operation::DelegateCall),
            other => Err(format!("invalid Safe operation: {other}")),
        }
    }
}

/// An unsigned multisig transaction proposal. Carries every field needed to
/// compute the SafeTx hash; the Safe address and chain id are supplied at
/// signing time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultisigTransactionProposal {
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub operation: Operation,
    pub safe_tx_gas: U256,
    pub base_gas: U256,
    pub gas_price: U256,
    pub gas_token: Option<Address>,
    pub refund_receiver: Option<Address>,
    pub nonce: U256,
    pub origin: Option<String>,
}

impl MultisigTransactionProposal {
    /// Converts to the SafeTx field set used for EIP-712 hashing. `None`
    /// gas-token / refund-receiver map to the zero address, matching the Safe
    /// contract's interpretation.
    pub(crate) fn to_safe_tx_fields(&self) -> SafeTxFields {
        SafeTxFields {
            to: self.to,
            value: self.value,
            data: self.data.clone(),
            operation: self.operation as u8,
            safe_tx_gas: self.safe_tx_gas,
            base_gas: self.base_gas,
            gas_price: self.gas_price,
            gas_token: self.gas_token.unwrap_or(Address::ZERO),
            refund_receiver: self.refund_receiver.unwrap_or(Address::ZERO),
            nonce: self.nonce,
        }
    }
}

/// Body for `POST /safes/{address}/multisig-transactions/`. Built from a signed
/// proposal (see `signing::state`). Decimal-string numerics avoid JS-unsafe
/// integer precision loss.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMultisigTransactionRequest {
    #[serde(serialize_with = "crate::serde_addr::serialize")]
    pub safe: Address,
    #[serde(serialize_with = "crate::serde_addr::serialize")]
    pub to: Address,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub value: U256,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Bytes>,
    pub operation: u8,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::serde_addr::option::serialize"
    )]
    pub gas_token: Option<Address>,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub safe_tx_gas: U256,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub base_gas: U256,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub gas_price: U256,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::serde_addr::option::serialize"
    )]
    pub refund_receiver: Option<Address>,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub nonce: U256,
    pub contract_transaction_hash: B256,
    #[serde(serialize_with = "crate::serde_addr::serialize")]
    pub sender: Address,
    /// Owner signature over the SafeTx digest, serialized as a `0x`-prefixed
    /// hex string. Per-owner format (NOT the concatenated `execTransaction`
    /// encoding).
    pub signature: Bytes,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

/// Response body for create (201) and `GET /multisig-transactions/{hash}/`
/// (`SafeMultisigTransactionResponseSerializerV2`). Only the fields we consume
/// are modeled; serde ignores the rest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigTransaction {
    pub safe: Address,
    pub to: Address,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub value: U256,
    #[serde(default)]
    pub data: Option<Bytes>,
    pub operation: u8,
    #[serde(default)]
    pub gas_token: Option<Address>,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub safe_tx_gas: U256,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub base_gas: U256,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub gas_price: U256,
    #[serde(default)]
    pub refund_receiver: Option<Address>,
    #[serde(with = "apyx_serde_ext::u256::from_dec")]
    pub nonce: U256,
    pub safe_tx_hash: B256,
    #[serde(default)]
    pub proposer: Option<Address>,
    #[serde(default)]
    pub executor: Option<Address>,
    pub is_executed: bool,
    #[serde(default)]
    pub is_successful: Option<bool>,
    pub confirmations_required: u64,
    #[serde(default)]
    pub confirmations: Vec<Confirmation>,
    pub trusted: bool,
    #[serde(default)]
    pub signatures: Option<String>,
}

impl MultisigTransaction {
    /// Whether enough owner confirmations have been collected to meet the
    /// Safe's threshold, i.e. the transaction is ready to execute on-chain.
    ///
    /// Uses `>=` (not `>`): a Safe with threshold `n` becomes executable once it
    /// has `n` confirmations, so e.g. a 1-of-1 Safe is confirmed at one
    /// signature.
    pub fn is_confirmed(&self) -> bool {
        self.confirmations.len() as u64 >= self.confirmations_required
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_casts_to_wire_integer() {
        assert_eq!(Operation::Call as u8, 0);
        assert_eq!(Operation::DelegateCall as u8, 1);
        assert_eq!(Operation::try_from(1u8), Ok(Operation::DelegateCall));
        assert!(Operation::try_from(2u8).is_err());
    }

    #[test]
    fn create_request_serializes_decimal_string_numerics() {
        let req = CreateMultisigTransactionRequest {
            safe: Address::with_last_byte(0x11),
            to: Address::with_last_byte(0x22),
            value: U256::from(1_000_000_000_000_000_000u64),
            data: None,
            operation: 0,
            gas_token: None,
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            refund_receiver: None,
            nonce: U256::from(7u64),
            contract_transaction_hash: B256::ZERO,
            sender: Address::with_last_byte(0x33),
            signature: Bytes::from(vec![0xab, 0xcd]),
            origin: Some("test".to_string()),
        };
        let v = serde_json::to_value(&req).expect("serialize");
        assert_eq!(v["value"], "1000000000000000000");
        assert_eq!(v["nonce"], "7");
        assert_eq!(v["operation"], 0);
        assert!(v.get("data").is_none(), "None data must be omitted");
        assert!(v.get("gasToken").is_none());
        assert_eq!(v["origin"], "test");
    }

    #[test]
    fn multisig_transaction_deserializes_string_numerics() {
        let json = serde_json::json!({
            "safe": "0x1111111111111111111111111111111111111111",
            "to": "0x2222222222222222222222222222222222222222",
            "value": "1000000000000000000",
            "data": null,
            "operation": 0,
            "gasToken": null,
            "safeTxGas": "0",
            "baseGas": "0",
            "gasPrice": "0",
            "refundReceiver": null,
            "nonce": "7",
            "safeTxHash": "0x3f8a1e46a40022cb6999456cabf559868588a28a256d60b0234ed7a2e21cf0d6",
            "proposer": "0x3333333333333333333333333333333333333333",
            "executor": null,
            "isExecuted": false,
            "isSuccessful": null,
            "confirmationsRequired": 2,
            "confirmations": [],
            "trusted": true,
            "signatures": null
        });
        let tx: MultisigTransaction = serde_json::from_value(json).expect("parse");
        assert_eq!(tx.value, U256::from(1_000_000_000_000_000_000u64));
        assert_eq!(tx.nonce, U256::from(7u64));
        assert_eq!(tx.confirmations_required, 2);
        assert!(!tx.is_executed);
        assert!(tx.trusted);
        // 0 confirmations, threshold 2 => not confirmed.
        assert!(!tx.is_confirmed());
    }

    fn confirmation(owner: u8) -> Confirmation {
        serde_json::from_value(serde_json::json!({
            "owner": Address::with_last_byte(owner),
            "submissionDate": "2026-01-01T00:00:00Z",
            "transactionHash": null,
            "signature": "0x00",
            "signatureType": "EOA"
        }))
        .expect("confirmation")
    }

    #[test]
    fn is_confirmed_uses_threshold_inclusively() {
        let mut tx: MultisigTransaction = serde_json::from_value(serde_json::json!({
            "safe": "0x1111111111111111111111111111111111111111",
            "to": "0x2222222222222222222222222222222222222222",
            "value": "0",
            "operation": 0,
            "safeTxGas": "0",
            "baseGas": "0",
            "gasPrice": "0",
            "nonce": "0",
            "safeTxHash": "0x3f8a1e46a40022cb6999456cabf559868588a28a256d60b0234ed7a2e21cf0d6",
            "isExecuted": false,
            "confirmationsRequired": 2,
            "confirmations": [],
            "trusted": false
        }))
        .expect("parse");

        assert!(!tx.is_confirmed());
        tx.confirmations.push(confirmation(0x01));
        assert!(!tx.is_confirmed());
        tx.confirmations.push(confirmation(0x02));
        assert!(tx.is_confirmed(), "exactly threshold confirmations counts");
    }
}
