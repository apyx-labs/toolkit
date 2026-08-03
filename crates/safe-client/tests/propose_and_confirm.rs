use alloy::{
    primitives::{Address, B256, Bytes, U256},
    signers::local::PrivateKeySigner,
};
use reqwest_middleware::ClientBuilder;
use safe_client::{
    MultisigTransactionProposal, auth_middleware, model::Operation, propose_and_confirm,
    rebase_middleware,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path_regex},
};

/// End-to-end: `propose_and_confirm` issues exactly one create call (which atomically
/// records the proposer's signature server-side) followed by one GET to
/// fetch the resulting transaction record. It must NOT also call
/// `POST /multisig-transactions/{hash}/confirmations/` — that endpoint is
/// reserved for additional owners adding their signatures, and calling it
/// for the proposer's own signature races the service's read-replica
/// indexing of the just-created tx (HTTP 404 in practice).
#[tokio::test]
async fn propose_creates_and_fetches_without_redundant_confirm() {
    let server = MockServer::start().await;
    let signer = PrivateKeySigner::from_bytes(&B256::from([0x11u8; 32])).expect("key");
    let safe = Address::with_last_byte(0xaa);
    let chain_id = 1u64;

    let proposal = MultisigTransactionProposal {
        to: Address::with_last_byte(0x22),
        value: U256::ZERO,
        data: Bytes::new(),
        operation: Operation::Call,
        safe_tx_gas: U256::ZERO,
        base_gas: U256::ZERO,
        gas_price: U256::ZERO,
        gas_token: None,
        refund_receiver: None,
        nonce: U256::ZERO,
        origin: None,
    };

    // Compute the hash the helper will derive, to build matching URLs/bodies.
    let signed_hash = {
        use safe_client::contract_transaction_hash;
        let s = proposal
            .clone()
            .sign(&signer, safe, chain_id)
            .await
            .expect("sign");
        let _ = contract_transaction_hash; // keep import used if refactored
        s.contract_transaction_hash
    };

    // POST create — exactly one call.
    Mock::given(method("POST"))
        .and(path_regex(r"^/safes/.*/multisig-transactions/$"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1)
        .mount(&server)
        .await;

    // POST confirm — must NEVER be called. The proposer's signature is in
    // the create body; an extra confirm call for that same signature is
    // redundant and races the service's index of the just-created tx,
    // yielding HTTP 404 in practice.
    Mock::given(method("POST"))
        .and(path_regex(r"^/multisig-transactions/.*/confirmations/$"))
        .respond_with(ResponseTemplate::new(201))
        .expect(0)
        .mount(&server)
        .await;

    // GET final — exactly one call to surface the record.
    Mock::given(method("GET"))
        .and(path_regex(r"^/multisig-transactions/.*/$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
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
            "safeTxHash": signed_hash,
            "proposer": signer.address(),
            "executor": null,
            "isExecuted": false,
            "isSuccessful": null,
            "confirmationsRequired": 1,
            "confirmations": [{
                "owner": signer.address(),
                "submissionDate": "2026-01-01T00:00:00Z",
                "transactionHash": null,
                "signature": "0x00",
                "signatureType": "EOA"
            }],
            "trusted": true,
            "signatures": null
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = ClientBuilder::new(reqwest::Client::new())
        .with(auth_middleware("k").expect("key"))
        .with(rebase_middleware(server.uri().parse().expect("url")))
        .build();

    let tx = propose_and_confirm(&client, safe, chain_id, proposal, &signer)
        .await
        .expect("propose_and_confirm");
    assert!(tx.trusted);
    assert_eq!(tx.confirmations.len(), 1);
}
