use std::time::Duration;

use apyx_reqwest_middleware::RouteLabel;
use reqwest::{Response, header::CONTENT_TYPE};
use reqwest_middleware::ClientWithMiddleware;
use serde::{Serialize, de::DeserializeOwned};

use super::DEFAULT_BASE_URL;
use crate::{
    DuneClient, Error,
    model::{
        CreateTableRequest, CreateTableResponse, ExecuteRequest, ExecuteResponse, ExecutionId,
        ExecutionStatus, InsertRowsResponse, LatestResultsResponse, QueryId, QueryParameter,
        QueryResult, ResultsResponse, TableRef,
    },
};

const NDJSON_CONTENT_TYPE: &str = "application/x-ndjson";

impl DuneClient for ClientWithMiddleware {
    async fn dune_execute_query(
        &self,
        query_id: QueryId,
        params: &[QueryParameter],
    ) -> Result<ExecuteResponse, Error> {
        let response = self
            .post(format!("{DEFAULT_BASE_URL}/query/{query_id}/execute"))
            .with_extension(RouteLabel("/query/{query_id}/execute"))
            .json(&ExecuteRequest::from(params))
            .send()
            .await
            .map_err(Error::ExecuteRequest)?;

        deserialize_json(response).await
    }

    async fn dune_get_results<T: DeserializeOwned>(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<ResultsResponse<T>, Error> {
        let response = self
            .get(format!(
                "{DEFAULT_BASE_URL}/execution/{execution_id}/results"
            ))
            .with_extension(RouteLabel("/execution/{execution_id}/results"))
            .send()
            .await
            .map_err(Error::ExecuteRequest)?;

        deserialize_json(response).await
    }

    async fn dune_get_latest_results<T: DeserializeOwned>(
        &self,
        query_id: QueryId,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<LatestResultsResponse<T>, Error> {
        let mut request = self
            .get(format!("{DEFAULT_BASE_URL}/query/{query_id}/results"))
            .with_extension(RouteLabel("/query/{query_id}/results"));
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit)]);
        }
        if let Some(offset) = offset {
            request = request.query(&[("offset", offset)]);
        }

        let response = request.send().await.map_err(Error::ExecuteRequest)?;
        deserialize_json(response).await
    }

    async fn dune_wait_for_results<T: DeserializeOwned>(
        &self,
        execution_id: &ExecutionId,
        poll_interval: Duration,
    ) -> Result<QueryResult<T>, Error> {
        loop {
            let response: ResultsResponse<T> = self.dune_get_results(execution_id).await?;
            let state = response.status.state();

            match response.status {
                ExecutionStatus::Completed { result } => return Ok(result),
                ExecutionStatus::Failed | ExecutionStatus::Cancelled | ExecutionStatus::Expired => {
                    return Err(Error::ExecutionEnded {
                        execution_id: response.execution_id,
                        state,
                    });
                }
                ExecutionStatus::Pending | ExecutionStatus::Executing => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    async fn dune_execute_and_wait<T: DeserializeOwned>(
        &self,
        query_id: QueryId,
        params: &[QueryParameter],
        poll_interval: Duration,
    ) -> Result<QueryResult<T>, Error> {
        let execution = self.dune_execute_query(query_id, params).await?;
        self.dune_wait_for_results(&execution.execution_id, poll_interval)
            .await
    }

    async fn dune_ping(&self) -> Result<(), Error> {
        let response = self
            .get(format!("{DEFAULT_BASE_URL}/auth/session/status"))
            .with_extension(RouteLabel("/auth/session/status"))
            .send()
            .await
            .map_err(Error::ExecuteRequest)?;

        check_status(response).await.map(drop)
    }

    async fn dune_insert_rows<T: Serialize>(
        &self,
        table: &TableRef,
        rows: &[T],
    ) -> Result<InsertRowsResponse, Error> {
        let mut body = Vec::new();
        for row in rows {
            serde_json::to_writer(&mut body, row).map_err(Error::SerializeRows)?;
            body.push(b'\n');
        }

        let response = self
            .post(format!(
                "{DEFAULT_BASE_URL}/uploads/{}/{}/insert",
                table.namespace, table.table_name
            ))
            .with_extension(RouteLabel("/uploads/{namespace}/{table_name}/insert"))
            .header(CONTENT_TYPE, NDJSON_CONTENT_TYPE)
            .body(body)
            .send()
            .await
            .map_err(Error::ExecuteRequest)?;

        deserialize_json(response).await
    }

    async fn dune_create_table(
        &self,
        req: &CreateTableRequest,
    ) -> Result<CreateTableResponse, Error> {
        let response = self
            .post(format!("{DEFAULT_BASE_URL}/uploads"))
            .with_extension(RouteLabel("/uploads"))
            .json(req)
            .send()
            .await
            .map_err(Error::ExecuteRequest)?;

        deserialize_json(response).await
    }

    async fn dune_delete_table(&self, table: &TableRef) -> Result<(), Error> {
        let response = self
            .delete(format!(
                "{DEFAULT_BASE_URL}/uploads/{}/{}",
                table.namespace, table.table_name
            ))
            .with_extension(RouteLabel("/uploads/{namespace}/{table_name}"))
            .send()
            .await
            .map_err(Error::ExecuteRequest)?;

        // 404 means the table doesn't exist, which is fine for delete.
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }

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
    use reqwest_middleware::ClientBuilder;
    use serde::Deserialize;
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_json, body_string, header, method, path},
    };

    use super::*;
    use crate::{auth_middleware, model::ParameterValue, rebase_middleware};

    const API_KEY: &str = "test-key";
    const POLL: Duration = Duration::from_millis(1);

    fn client(server: &MockServer) -> ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new())
            .with(auth_middleware(API_KEY).expect("valid api key"))
            .with(rebase_middleware(
                server.uri().parse().expect("valid mock server url"),
            ))
            .build()
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct Row {
        wallet: String,
        balance: u64,
    }

    #[tokio::test]
    async fn execute_query_sends_auth_header_and_parameters() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/query/123/execute"))
            .and(header("x-dune-api-key", API_KEY))
            .and(body_json(json!({
                "query_parameters": { "from": "2024-01-01 00:00:00", "limit": "10" }
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-1",
                "state": "QUERY_STATE_PENDING"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let from = "2024-01-01T00:00:00Z"
            .parse::<chrono::DateTime<chrono::Utc>>()
            .expect("valid datetime");
        let params = [
            QueryParameter::new("from", ParameterValue::Date(from)),
            QueryParameter::new("limit", ParameterValue::Number(10.0)),
        ];
        let response = client(&server)
            .dune_execute_query(QueryId(123), &params)
            .await
            .expect("execute succeeds");

        assert_eq!(response.execution_id, ExecutionId::from("exec-1"));
        assert_eq!(response.state, crate::model::ExecutionState::Pending);
    }

    #[tokio::test]
    async fn execute_query_without_parameters_omits_map() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/query/7/execute"))
            .and(body_json(json!({})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-7",
                "state": "QUERY_STATE_EXECUTING"
            })))
            .expect(1)
            .mount(&server)
            .await;

        client(&server)
            .dune_execute_query(QueryId(7), &[])
            .await
            .expect("execute succeeds");
    }

    #[tokio::test]
    async fn execute_and_wait_polls_until_complete() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/query/123/execute"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-1",
                "state": "QUERY_STATE_PENDING"
            })))
            .mount(&server)
            .await;

        // First poll returns a pending execution, the second one completes.
        Mock::given(method("GET"))
            .and(path("/execution/exec-1/results"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-1",
                "state": "QUERY_STATE_PENDING"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/execution/exec-1/results"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-1",
                "state": "QUERY_STATE_COMPLETED",
                "result": {
                    "rows": [
                        { "wallet": "0xabc", "balance": 100 },
                        { "wallet": "0xdef", "balance": 250 }
                    ],
                    "metadata": { "column_names": ["wallet", "balance"], "row_count": 2 }
                }
            })))
            .mount(&server)
            .await;

        let result: QueryResult<Row> = client(&server)
            .dune_execute_and_wait(QueryId(123), &[], POLL)
            .await
            .expect("execution completes");

        assert_eq!(
            result.rows,
            vec![
                Row {
                    wallet: "0xabc".into(),
                    balance: 100
                },
                Row {
                    wallet: "0xdef".into(),
                    balance: 250
                },
            ]
        );
        assert_eq!(result.metadata.row_count, 2);
        assert_eq!(result.metadata.column_names, vec!["wallet", "balance"]);
    }

    #[tokio::test]
    async fn get_latest_results_sends_pagination_params_and_never_posts() {
        use wiremock::matchers::query_param;

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/query/7695360/results"))
            .and(header("x-dune-api-key", API_KEY))
            .and(query_param("limit", "100"))
            .and(query_param("offset", "200"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-1",
                "state": "QUERY_STATE_COMPLETED",
                "execution_ended_at": "2026-06-10T12:00:00Z",
                "result": {
                    "rows": [{ "wallet": "0xabc", "balance": 100 }],
                    "metadata": { "column_names": ["wallet", "balance"], "row_count": 1 }
                }
            })))
            .expect(1)
            .mount(&server)
            .await;
        // The exporter use case must never execute queries (costs credits).
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .expect(0)
            .mount(&server)
            .await;

        let response: LatestResultsResponse<Row> = client(&server)
            .dune_get_latest_results(QueryId(7695360), Some(100), Some(200))
            .await
            .expect("fetch succeeds");

        let ExecutionStatus::Completed { result } = response.status else {
            panic!("expected completed");
        };
        assert_eq!(result.rows.len(), 1);
    }

    #[tokio::test]
    async fn get_latest_results_omits_absent_params() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/query/7/results"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-7",
                "state": "QUERY_STATE_PENDING"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let response: LatestResultsResponse<serde_json::Value> = client(&server)
            .dune_get_latest_results(QueryId(7), None, None)
            .await
            .expect("fetch succeeds");
        assert_eq!(
            response.status.state(),
            crate::model::ExecutionState::Pending
        );
    }

    #[tokio::test]
    async fn wait_for_results_errors_on_terminal_failure_state() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/execution/exec-9/results"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "execution_id": "exec-9",
                "state": "QUERY_STATE_FAILED"
            })))
            .mount(&server)
            .await;

        let error = client(&server)
            .dune_wait_for_results::<serde_json::Value>(&ExecutionId::from("exec-9"), POLL)
            .await
            .expect_err("failed execution is an error");

        assert!(
            matches!(
                &error,
                Error::ExecutionEnded { execution_id, state }
                    if execution_id == &ExecutionId::from("exec-9")
                        && *state == crate::model::ExecutionState::Failed
            ),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn ping_maps_non_2xx_to_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/auth/session/status"))
            .respond_with(ResponseTemplate::new(401).set_body_string("invalid API key"))
            .mount(&server)
            .await;

        let error = client(&server)
            .dune_ping()
            .await
            .expect_err("401 is an error");

        assert!(
            matches!(
                &error,
                Error::Api { status, body }
                    if *status == reqwest::StatusCode::UNAUTHORIZED && body == "invalid API key"
            ),
            "unexpected error: {error}"
        );
    }

    #[tokio::test]
    async fn insert_rows_sends_ndjson_body() {
        #[derive(Serialize)]
        struct Upload {
            wallet: &'static str,
            balance: u64,
        }

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/uploads/my_team/balances/insert"))
            .and(header("content-type", "application/x-ndjson"))
            .and(body_string(
                "{\"wallet\":\"0xabc\",\"balance\":100}\n{\"wallet\":\"0xdef\",\"balance\":250}\n",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "rows_written": 2,
                "bytes_written": 68
            })))
            .expect(1)
            .mount(&server)
            .await;

        let rows = [
            Upload {
                wallet: "0xabc",
                balance: 100,
            },
            Upload {
                wallet: "0xdef",
                balance: 250,
            },
        ];
        let response = client(&server)
            .dune_insert_rows(&TableRef::new("my_team", "balances"), &rows)
            .await
            .expect("insert succeeds");

        assert_eq!(response.rows_written, 2);
        assert_eq!(response.bytes_written, 68);
    }

    #[tokio::test]
    async fn create_table_sends_schema() {
        use crate::model::{Column, ColumnType};

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/uploads"))
            .and(body_json(json!({
                "namespace": "my_team",
                "table_name": "balances",
                "schema": [
                    { "name": "wallet", "type": "varchar", "nullable": false },
                    { "name": "balance", "type": "uint256", "nullable": true }
                ],
                "description": "wallet balances",
                "is_private": false
            })))
            .respond_with(ResponseTemplate::new(201).set_body_json(json!({
                "namespace": "my_team",
                "table_name": "balances",
                "full_name": "dune.my_team.balances",
                "example_query": "select * from dune.my_team.balances limit 10",
                "already_existed": false,
                "message": "Table created successfully"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let request = CreateTableRequest {
            namespace: "my_team".into(),
            table_name: "balances".into(),
            schema: vec![
                Column::new("wallet", ColumnType::Varchar, false),
                Column::new("balance", ColumnType::Uint256, true),
            ],
            description: Some("wallet balances".into()),
            is_private: false,
        };
        let response = client(&server)
            .dune_create_table(&request)
            .await
            .expect("create succeeds");

        assert_eq!(response.full_name, "dune.my_team.balances");
        assert!(!response.already_existed);
    }

    #[tokio::test]
    async fn delete_table_treats_404_as_success() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/uploads/my_team/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        client(&server)
            .dune_delete_table(&TableRef::new("my_team", "missing"))
            .await
            .expect("missing table delete is ok");
    }

    #[tokio::test]
    async fn delete_table_propagates_other_errors() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/uploads/my_team/balances"))
            .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
            .mount(&server)
            .await;

        let error = client(&server)
            .dune_delete_table(&TableRef::new("my_team", "balances"))
            .await
            .expect_err("500 is an error");

        assert!(matches!(&error, Error::Api { status, .. }
            if *status == reqwest::StatusCode::INTERNAL_SERVER_ERROR));
    }
}
