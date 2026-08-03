use alloy::primitives::{Address, B256, Bytes};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Safe signature type as reported by the Transaction Service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SignatureType {
    Eoa,
    EthSign,
    ApprovedHash,
    ContractSignature,
    #[serde(other)]
    Unknown,
}

/// A single owner confirmation (signature) on a multisig transaction.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Confirmation {
    pub owner: Address,
    pub submission_date: DateTime<Utc>,
    #[serde(default)]
    pub transaction_hash: Option<B256>,
    /// Raw signature bytes. Variable-length: a 65-byte ECDSA signature for an
    /// EOA, or longer/shorter for EIP-1271 contract and approved-hash entries.
    pub signature: Bytes,
    pub signature_type: SignatureType,
}

/// Paginated list returned by the confirmations endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ConfirmationList {
    pub count: u64,
    #[serde(default)]
    pub next: Option<String>,
    #[serde(default)]
    pub previous: Option<String>,
    pub results: Vec<Confirmation>,
}

/// Body for `POST /multisig-transactions/{hash}/confirmations/`.
#[derive(Debug, Clone, Serialize)]
pub struct ConfirmRequest {
    /// Owner signature over the SafeTx digest, serialized as a `0x`-prefixed
    /// hex string (typically a 65-byte ECDSA signature).
    pub signature: Bytes,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmation_deserializes() {
        let json = serde_json::json!({
            "owner": "0x1111111111111111111111111111111111111111",
            "submissionDate": "2026-01-01T00:00:00Z",
            "transactionHash": null,
            "signature": "0xabcd",
            "signatureType": "EOA"
        });
        let c: Confirmation = serde_json::from_value(json).expect("parse");
        assert_eq!(c.signature_type, SignatureType::Eoa);
        assert_eq!(c.signature, Bytes::from(vec![0xab, 0xcd]));
        assert!(c.transaction_hash.is_none());
    }

    #[test]
    fn confirmation_list_deserializes() {
        let json = serde_json::json!({
            "count": 0,
            "next": null,
            "previous": null,
            "results": []
        });
        let list: ConfirmationList = serde_json::from_value(json).expect("parse");
        assert_eq!(list.count, 0);
        assert!(list.results.is_empty());
    }

    #[test]
    fn unknown_signature_type_is_tolerated() {
        let json = serde_json::json!({
            "owner": "0x1111111111111111111111111111111111111111",
            "submissionDate": "2026-01-01T00:00:00Z",
            "signature": "0x",
            "signatureType": "SOMETHING_NEW"
        });
        let c: Confirmation = serde_json::from_value(json).expect("parse");
        assert_eq!(c.signature_type, SignatureType::Unknown);
    }
}
