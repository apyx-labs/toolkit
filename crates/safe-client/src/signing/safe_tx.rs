use alloy::{
    primitives::{Address, B256, Bytes, U256},
    sol,
    sol_types::{Eip712Domain, SolStruct},
};

sol! {
    /// EIP-712 `SafeTx` struct hashed and signed by Safe owners. Field order
    /// and types match the Safe contract's `SAFE_TX_TYPEHASH`.
    #[allow(missing_docs)]
    struct SafeTx {
        address to;
        uint256 value;
        bytes data;
        uint8 operation;
        uint256 safeTxGas;
        uint256 baseGas;
        uint256 gasPrice;
        address gasToken;
        address refundReceiver;
        uint256 nonce;
    }
}

/// The plain (snake_case) field set of a SafeTx, shared between the signing and
/// model layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeTxFields {
    pub to: Address,
    pub value: U256,
    pub data: Bytes,
    pub operation: u8,
    pub safe_tx_gas: U256,
    pub base_gas: U256,
    pub gas_price: U256,
    pub gas_token: Address,
    pub refund_receiver: Address,
    pub nonce: U256,
}

impl SafeTxFields {
    /// Builds the `sol!`-generated EIP-712 struct from these fields.
    pub(crate) fn to_sol(&self) -> SafeTx {
        SafeTx {
            to: self.to,
            value: self.value,
            data: self.data.clone(),
            operation: self.operation,
            safeTxGas: self.safe_tx_gas,
            baseGas: self.base_gas,
            gasPrice: self.gas_price,
            gasToken: self.gas_token,
            refundReceiver: self.refund_receiver,
            nonce: self.nonce,
        }
    }
}

/// Builds the Safe EIP-712 domain. Safe uses only `chainId` and
/// `verifyingContract` (the Safe address); `name` and `version` are absent.
fn safe_domain(safe: Address, chain_id: u64) -> Eip712Domain {
    Eip712Domain::new(None, None, Some(U256::from(chain_id)), Some(safe), None)
}

/// Computes the Safe contract transaction hash (the EIP-712 signing digest,
/// a.k.a. `safeTxHash`) that owners sign and the service stores as
/// `contractTransactionHash`.
pub fn contract_transaction_hash(fields: &SafeTxFields, safe: Address, chain_id: u64) -> B256 {
    fields
        .to_sol()
        .eip712_signing_hash(&safe_domain(safe, chain_id))
}

#[cfg(test)]
mod tests {
    use alloy::{
        primitives::{Address, U256, b256, bytes},
        sol_types::SolStruct,
    };

    use super::*;

    #[test]
    fn safe_tx_type_hash_matches_canonical() {
        let fields = SafeTxFields {
            to: Address::ZERO,
            value: U256::ZERO,
            data: bytes!(""),
            operation: 0,
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            gas_token: Address::ZERO,
            refund_receiver: Address::ZERO,
            nonce: U256::ZERO,
        };
        let safe_tx = fields.to_sol();
        assert_eq!(
            safe_tx.eip712_type_hash(),
            b256!("0xbb8310d486368db6bd6f849402fdd73ad53d316b5a4b2644ad6efe0f941286d8")
        );
    }

    #[test]
    fn hash_is_deterministic_and_domain_sensitive() {
        let fields = SafeTxFields {
            to: Address::with_last_byte(0xab),
            value: U256::from(1u64),
            data: bytes!("deadbeef"),
            operation: 0,
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            gas_token: Address::ZERO,
            refund_receiver: Address::ZERO,
            nonce: U256::from(5u64),
        };
        let safe = Address::with_last_byte(0x11);

        let h1 = contract_transaction_hash(&fields, safe, 1);
        let h1_again = contract_transaction_hash(&fields, safe, 1);
        let h_other_chain = contract_transaction_hash(&fields, safe, 8453);

        assert_eq!(h1, h1_again);
        assert_ne!(
            h1, h_other_chain,
            "chain id must affect the domain separator"
        );
    }
}
