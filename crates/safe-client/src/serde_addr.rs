//! Serde helpers for serializing `alloy::primitives::Address` as EIP-55
//! checksum-cased strings, as required by the Safe Transaction Service v2
//! request payload schema. Alloy's default Address Serialize emits lowercase
//! hex, which the service rejects with 422 ("Address X is not checksumed").

use alloy::primitives::Address;
use serde::Serializer;

pub fn serialize<S: Serializer>(a: &Address, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&a.to_checksum(None))
}

pub mod option {
    use alloy::primitives::Address;
    use serde::Serializer;

    pub fn serialize<S: Serializer>(a: &Option<Address>, s: S) -> Result<S::Ok, S::Error> {
        match a {
            Some(addr) => s.serialize_str(&addr.to_checksum(None)),
            None => s.serialize_none(),
        }
    }
}
