//! Serde serializers for [`alloy_primitives::Address`] encoded as EIP-55
//! checksum-cased strings.

use alloy_primitives::Address;
use serde::Serializer;

/// Serializes an [`Address`] as its EIP-55 checksum-cased string.
pub fn serialize<S: Serializer>(address: &Address, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&address.to_checksum(None))
}

/// Serde serializers for optional EIP-55 checksum-cased addresses.
pub mod option {
    use alloy_primitives::Address;
    use serde::Serializer;

    /// Serializes `Some(Address)` as an EIP-55 checksum-cased string and
    /// `None` as `null`.
    pub fn serialize<S: Serializer>(
        address: &Option<Address>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match address {
            Some(address) => serializer.serialize_str(&address.to_checksum(None)),
            None => serializer.serialize_none(),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::Address;
    use serde::Serialize;

    #[derive(Serialize)]
    struct AddressWrap {
        #[serde(serialize_with = "super::serialize")]
        address: Address,
    }

    #[derive(Serialize)]
    struct OptionalAddressWrap {
        #[serde(serialize_with = "super::option::serialize")]
        address: Option<Address>,
    }

    #[test]
    fn serializes_an_eip_55_checksum_address() {
        let address = "0x52908400098527886E0F7030069857D2E4169EE7"
            .parse()
            .expect("valid address");
        let json = serde_json::to_string(&AddressWrap { address }).expect("serialize");

        assert_eq!(
            json,
            r#"{"address":"0x52908400098527886E0F7030069857D2E4169EE7"}"#
        );
    }

    #[test]
    fn serializes_some_optional_address_as_eip_55_checksum_address() {
        let address = "0x5Aeda56215b167893e80B4fE645BA6d5Bab767DE"
            .parse()
            .expect("valid address");
        let json = serde_json::to_string(&OptionalAddressWrap {
            address: Some(address),
        })
        .expect("serialize");

        assert_eq!(
            json,
            r#"{"address":"0x5AEDA56215b167893e80B4fE645BA6d5Bab767DE"}"#
        );
    }

    #[test]
    fn serializes_none_optional_address_as_null() {
        let json =
            serde_json::to_string(&OptionalAddressWrap { address: None }).expect("serialize");

        assert_eq!(json, r#"{"address":null}"#);
    }
}
