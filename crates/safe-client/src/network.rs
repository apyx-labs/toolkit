//! Safe Transaction Service base-URL construction over [`alloy_chains::NamedChain`].

use alloy_chains::NamedChain;
use url::Url;

use crate::Error;

/// Canonical base URL that [`SafeClient`](crate::SafeClient) builds requests
/// against. When the `reqwest` feature is enabled,
/// [`rebase_middleware`](crate::rebase_middleware) rewrites this prefix to the
/// caller's chosen network (or a mock server in tests).
pub const DEFAULT_BASE_URL: &str = "https://api.safe.global/tx-service/eth/api/v2";

/// Maps a [`NamedChain`] to the EIP-3770 short name the Safe gateway expects.
pub trait SafeNetworkSlug {
    /// The Safe gateway path slug for this network, if supported.
    fn safe_slug(&self) -> Option<&'static str>;
}

impl SafeNetworkSlug for NamedChain {
    fn safe_slug(&self) -> Option<&'static str> {
        match self {
            NamedChain::Mainnet => Some("eth"),
            NamedChain::Base => Some("base"),
            NamedChain::Arbitrum => Some("arb1"),
            NamedChain::Sepolia => Some("sep"),
            _ => None,
        }
    }
}

/// Builds the Safe Transaction Service v2 base URL for a supported network.
pub fn base_url(network: NamedChain) -> Result<Url, Error> {
    let slug = network
        .safe_slug()
        .ok_or(Error::UnsupportedNetwork(network))?;
    let raw = format!("https://api.safe.global/tx-service/{slug}/api/v2");
    Url::parse(&raw).map_err(Error::InvalidBaseUrl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_base_url_is_eth_slug() {
        let url = base_url(NamedChain::Mainnet).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://api.safe.global/tx-service/eth/api/v2"
        );
    }

    #[test]
    fn base_url_uses_eip3770_slugs() {
        assert!(
            base_url(NamedChain::Base)
                .expect("url")
                .as_str()
                .contains("/base/")
        );
        assert!(
            base_url(NamedChain::Arbitrum)
                .expect("url")
                .as_str()
                .contains("/arb1/")
        );
        assert!(
            base_url(NamedChain::Sepolia)
                .expect("url")
                .as_str()
                .contains("/sep/")
        );
    }

    #[test]
    fn safe_slugs_match_eip3770() {
        assert_eq!(NamedChain::Mainnet.safe_slug(), Some("eth"));
        assert_eq!(NamedChain::Base.safe_slug(), Some("base"));
        assert_eq!(NamedChain::Arbitrum.safe_slug(), Some("arb1"));
        assert_eq!(NamedChain::Sepolia.safe_slug(), Some("sep"));
    }

    #[test]
    fn unsupported_network_errors() {
        let err = base_url(NamedChain::Optimism).expect_err("unsupported");
        assert!(matches!(
            err,
            Error::UnsupportedNetwork(NamedChain::Optimism)
        ));
    }
}
