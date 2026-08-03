//! `SafeClient` impl for `ClientWithMiddleware`.

use alloy::primitives::{Address, B256, Bytes};
use apyx_reqwest_middleware::RouteLabel;
use reqwest::Response;
use reqwest_middleware::ClientWithMiddleware;
use serde::de::DeserializeOwned;

use crate::{
    DEFAULT_BASE_URL, Error, SafeClient,
    model::{
        ConfirmRequest, ConfirmationList, CreateMultisigTransactionRequest, MultisigTransaction,
    },
};

impl SafeClient for ClientWithMiddleware {
    async fn safe_create_multisig_transaction(
        &self,
        safe: Address,
        request: &CreateMultisigTransactionRequest,
    ) -> Result<(), Error> {
        let response = self
            .post(format!(
                "{DEFAULT_BASE_URL}/safes/{safe}/multisig-transactions/"
            ))
            .with_extension(RouteLabel("/safes/{address}/multisig-transactions/"))
            .json(request)
            .send()
            .await
            .map_err(Error::SendRequest)?;

        check_status(response).await.map(drop)
    }

    async fn safe_get_multisig_transaction(
        &self,
        safe_tx_hash: B256,
    ) -> Result<MultisigTransaction, Error> {
        let response = self
            .get(format!(
                "{DEFAULT_BASE_URL}/multisig-transactions/{safe_tx_hash}/"
            ))
            .with_extension(RouteLabel("/multisig-transactions/{safe_tx_hash}/"))
            .send()
            .await
            .map_err(Error::SendRequest)?;

        deserialize_json(response).await
    }

    async fn safe_list_multisig_confirmations(
        &self,
        safe_tx_hash: B256,
    ) -> Result<ConfirmationList, Error> {
        let response = self
            .get(format!(
                "{DEFAULT_BASE_URL}/multisig-transactions/{safe_tx_hash}/confirmations/"
            ))
            .with_extension(RouteLabel(
                "/multisig-transactions/{safe_tx_hash}/confirmations/",
            ))
            .send()
            .await
            .map_err(Error::SendRequest)?;

        deserialize_json(response).await
    }

    async fn safe_confirm_multisig_transaction(
        &self,
        safe_tx_hash: B256,
        signature: Bytes,
    ) -> Result<(), Error> {
        let body = ConfirmRequest { signature };
        let response = self
            .post(format!(
                "{DEFAULT_BASE_URL}/multisig-transactions/{safe_tx_hash}/confirmations/"
            ))
            .with_extension(RouteLabel(
                "/multisig-transactions/{safe_tx_hash}/confirmations/",
            ))
            .json(&body)
            .send()
            .await
            .map_err(Error::SendRequest)?;

        check_status(response).await.map(drop)
    }
}

/// Maps non-2xx responses to [`Error::Api`], capturing the response body.
async fn check_status(response: Response) -> Result<Response, Error> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().await.unwrap_or_default();
    Err(Error::Api { status, body })
}

async fn deserialize_json<T: DeserializeOwned>(response: Response) -> Result<T, Error> {
    check_status(response)
        .await?
        .json()
        .await
        .map_err(Error::DeserializeResponse)
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, Bytes, U256, b256};
    use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{header, method, path},
    };

    use crate::{
        Error, SafeClient, auth_middleware, model::CreateMultisigTransactionRequest,
        rebase_middleware,
    };

    const API_KEY: &str = "test-key";
    const SAFE_TX_HASH: B256 =
        b256!("0x3f8a1e46a40022cb6999456cabf559868588a28a256d60b0234ed7a2e21cf0d6");

    fn client(server: &MockServer) -> ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new())
            .with(auth_middleware(API_KEY).expect("valid key"))
            .with(rebase_middleware(server.uri().parse().expect("mock url")))
            .build()
    }

    fn sample_request(safe: Address) -> CreateMultisigTransactionRequest {
        CreateMultisigTransactionRequest {
            safe,
            to: Address::with_last_byte(0x22),
            value: U256::ZERO,
            data: None,
            operation: 0,
            gas_token: None,
            safe_tx_gas: U256::ZERO,
            base_gas: U256::ZERO,
            gas_price: U256::ZERO,
            refund_receiver: None,
            nonce: U256::ZERO,
            contract_transaction_hash: SAFE_TX_HASH,
            sender: Address::with_last_byte(0x33),
            signature: Bytes::from(vec![0xab, 0xcd]),
            origin: None,
        }
    }

    fn tx_response_json(safe: Address) -> serde_json::Value {
        serde_json::json!({
            "safe": safe,
            "to": "0x2222222222222222222222222222222222222222",
            "value": "0",
            "data": null,
            "operation": 0,
            "gasToken": null,
            "safeTxGas": "0",
            "baseGas": "0",
            "gasPrice": "0",
            "refundReceiver": null,
            "nonce": "0",
            "safeTxHash": SAFE_TX_HASH,
            "proposer": "0x3333333333333333333333333333333333333333",
            "executor": null,
            "isExecuted": false,
            "isSuccessful": null,
            "confirmationsRequired": 1,
            "confirmations": [],
            "trusted": true,
            "signatures": null
        })
    }

    #[tokio::test]
    async fn create_sends_auth_and_returns_ok_on_201() {
        let server = MockServer::start().await;
        let safe = Address::with_last_byte(0x11);
        Mock::given(method("POST"))
            .and(path(format!("/safes/{safe}/multisig-transactions/")))
            .and(header("authorization", "Bearer test-key"))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&server)
            .await;

        client(&server)
            .safe_create_multisig_transaction(safe, &sample_request(safe))
            .await
            .expect("create");
    }

    #[tokio::test]
    async fn get_deserializes_transaction() {
        let server = MockServer::start().await;
        let safe = Address::with_last_byte(0x11);
        Mock::given(method("GET"))
            .and(path(format!("/multisig-transactions/{SAFE_TX_HASH}/")))
            .respond_with(ResponseTemplate::new(200).set_body_json(tx_response_json(safe)))
            .expect(1)
            .mount(&server)
            .await;

        let tx = client(&server)
            .safe_get_multisig_transaction(SAFE_TX_HASH)
            .await
            .expect("get");
        assert_eq!(tx.safe_tx_hash, SAFE_TX_HASH);
        assert_eq!(tx.confirmations_required, 1);
    }

    #[tokio::test]
    async fn list_confirmations_deserializes() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!(
                "/multisig-transactions/{SAFE_TX_HASH}/confirmations/"
            )))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "count": 1,
                "next": null,
                "previous": null,
                "results": [{
                    "owner": "0x1111111111111111111111111111111111111111",
                    "submissionDate": "2026-01-01T00:00:00Z",
                    "transactionHash": null,
                    "signature": "0xabcd",
                    "signatureType": "EOA"
                }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let list = client(&server)
            .safe_list_multisig_confirmations(SAFE_TX_HASH)
            .await
            .expect("list");
        assert_eq!(list.count, 1);
        assert_eq!(list.results.len(), 1);
    }

    #[tokio::test]
    async fn confirm_posts_signature() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path(format!(
                "/multisig-transactions/{SAFE_TX_HASH}/confirmations/"
            )))
            .and(wiremock::matchers::body_json(
                serde_json::json!({ "signature": "0xabcd" }),
            ))
            .respond_with(ResponseTemplate::new(201))
            .expect(1)
            .mount(&server)
            .await;

        client(&server)
            .safe_confirm_multisig_transaction(SAFE_TX_HASH, Bytes::from(vec![0xab, 0xcd]))
            .await
            .expect("confirm");
    }

    #[tokio::test]
    async fn non_2xx_maps_to_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(format!("/multisig-transactions/{SAFE_TX_HASH}/")))
            .respond_with(ResponseTemplate::new(422).set_body_string("bad signature"))
            .mount(&server)
            .await;

        let err = client(&server)
            .safe_get_multisig_transaction(SAFE_TX_HASH)
            .await
            .expect_err("should error");
        match err {
            Error::Api { status, body } => {
                assert_eq!(status.as_u16(), 422);
                assert_eq!(body, "bad signature");
            }
            other => panic!("expected Api error, got {other:?}"),
        }
    }
}
