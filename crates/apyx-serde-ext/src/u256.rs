//! (De)serializers for [`alloy_primitives::U256`].
//!
//! Per-encoding submodules are intended for use with `#[serde(with = ...)]`,
//! e.g. `#[serde(with = "apyx_serde_ext::u256::from_dec")]`.

/// Encodes [`alloy_primitives::U256`] as a base-10 string, matching the
/// `TokenAmount` / `BigUint` encoding used by many JSON APIs.
pub mod from_dec {
    use alloy_primitives::U256;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(value: &U256, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<U256, D::Error> {
        let raw = String::deserialize(deserializer)?;
        U256::from_str_radix(&raw, 10).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::U256;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct Wrap {
        #[serde(with = "super::from_dec")]
        v: U256,
    }

    #[test]
    fn roundtrips_decimal_string() {
        let w = Wrap {
            v: U256::from(1_000_000_000_000_000_000u64),
        };
        let json = serde_json::to_string(&w).expect("serialize");
        assert_eq!(json, r#"{"v":"1000000000000000000"}"#);
        let back: Wrap = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, w);
    }

    #[test]
    fn rejects_non_decimal() {
        let err = serde_json::from_str::<Wrap>(r#"{"v":"0x01"}"#).expect_err("hex must fail");
        let _ = err;
    }
}
