//! Safe Transaction Service base-URL construction over the shared
//! [`common::types::Network`] enum.

use common::types::Network;
use reqwest::Url;

use crate::Error;

/// Canonical base URL that [`SafeClient`](crate::SafeClient) builds requests
/// against. [`rebase_middleware`](crate::rebase_middleware) rewrites this prefix
/// to the caller's chosen network (or a mock server in tests).
pub const DEFAULT_BASE_URL: &str = "https://api.safe.global/tx-service/eth/api/v2";

/// Maps a [`Network`] to the EIP-3770 short name the Safe gateway expects in its
/// path, e.g. `eth` in `https://api.safe.global/tx-service/eth/api/v2`.
///
/// This Safe-specific slug lives in the client crate (rather than on the shared
/// `Network` type) so `common` stays free of service-specific naming.
pub trait SafeNetworkSlug {
    /// The Safe gateway path slug for this network.
    fn safe_slug(&self) -> &'static str;
}

impl SafeNetworkSlug for Network {
    fn safe_slug(&self) -> &'static str {
        match self {
            Network::Mainnet => "eth",
            Network::Base => "base",
            Network::ArbitrumOne => "arb1",
            Network::Sepolia => "sep",
        }
    }
}

/// Builds the Safe Transaction Service v2 base URL for a network, e.g.
/// `https://api.safe.global/tx-service/base/api/v2`.
pub fn base_url(network: Network) -> Result<Url, Error> {
    let raw = format!(
        "https://api.safe.global/tx-service/{}/api/v2",
        network.safe_slug()
    );
    Url::parse(&raw).map_err(Error::InvalidBaseUrl)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_base_url_is_eth_slug() {
        let url = base_url(Network::Mainnet).expect("valid url");
        assert_eq!(
            url.as_str(),
            "https://api.safe.global/tx-service/eth/api/v2"
        );
    }

    #[test]
    fn base_url_uses_eip3770_slugs() {
        assert!(
            base_url(Network::Base)
                .expect("url")
                .as_str()
                .contains("/base/")
        );
        assert!(
            base_url(Network::ArbitrumOne)
                .expect("url")
                .as_str()
                .contains("/arb1/")
        );
        assert!(
            base_url(Network::Sepolia)
                .expect("url")
                .as_str()
                .contains("/sep/")
        );
    }

    #[test]
    fn safe_slugs_match_eip3770() {
        assert_eq!(Network::Mainnet.safe_slug(), "eth");
        assert_eq!(Network::Base.safe_slug(), "base");
        assert_eq!(Network::ArbitrumOne.safe_slug(), "arb1");
        assert_eq!(Network::Sepolia.safe_slug(), "sep");
    }
}
