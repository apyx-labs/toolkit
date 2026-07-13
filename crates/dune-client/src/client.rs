use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};

use crate::{
    Error,
    model::{
        CreateTableRequest, CreateTableResponse, ExecuteResponse, ExecutionId,
        InsertRowsResponse, LatestResultsResponse, QueryId, QueryParameter, QueryResult,
        TableRef,
    },
};

/// Base URL of the Dune Analytics API. Requests are built against this URL;
/// [`rebase_middleware`](crate::rebase_middleware) can rewrite it when the
/// `reqwest` feature is enabled.
pub const DEFAULT_BASE_URL: &str = "https://api.dune.com/api/v1";

/// Operations against the Dune Analytics API.
///
/// With the `reqwest` feature, implemented for `reqwest_middleware::ClientWithMiddleware`;
/// authentication is provided by registering `auth_middleware` on the client
/// rather than by a bespoke client type.
#[allow(async_fn_in_trait)]
pub trait DuneClient {
    /// Triggers execution of a Dune query by ID with optional parameters.
    ///
    /// Prefixed with `dune_` so multiple client traits can be implemented on the
    /// same HTTP client without method-name collisions.
    async fn dune_execute_query(
        &self,
        query_id: QueryId,
        params: &[QueryParameter],
    ) -> Result<ExecuteResponse, Error>;

    /// Fetches the current results of an execution without polling.
    async fn dune_get_results<T: DeserializeOwned>(
        &self,
        execution_id: &ExecutionId,
    ) -> Result<crate::model::ResultsResponse<T>, Error>;

    /// Fetches the latest saved results of a query (`GET /query/{id}/results`)
    /// without triggering an execution. `limit`/`offset` drive pagination;
    /// follow [`LatestResultsResponse::next_offset`] for subsequent pages.
    async fn dune_get_latest_results<T: DeserializeOwned>(
        &self,
        query_id: QueryId,
        limit: Option<u64>,
        offset: Option<u64>,
    ) -> Result<LatestResultsResponse<T>, Error>;

    /// Polls for execution results until the execution reaches a terminal
    /// state. Terminal failure states return [`Error::ExecutionEnded`].
    async fn dune_wait_for_results<T: DeserializeOwned>(
        &self,
        execution_id: &ExecutionId,
        poll_interval: Duration,
    ) -> Result<QueryResult<T>, Error>;

    /// Triggers a query and polls until results are ready.
    async fn dune_execute_and_wait<T: DeserializeOwned>(
        &self,
        query_id: QueryId,
        params: &[QueryParameter],
        poll_interval: Duration,
    ) -> Result<QueryResult<T>, Error>;

    /// Checks connectivity to the Dune API.
    async fn dune_ping(&self) -> Result<(), Error>;

    /// Appends rows to an existing Dune table as NDJSON.
    async fn dune_insert_rows<T: Serialize>(
        &self,
        table: &TableRef,
        rows: &[T],
    ) -> Result<InsertRowsResponse, Error>;

    /// Creates an empty Dune table with a defined schema.
    async fn dune_create_table(&self, req: &CreateTableRequest) -> Result<CreateTableResponse, Error>;

    /// Permanently deletes a Dune table. Succeeds if the table does not exist.
    async fn dune_delete_table(&self, table: &TableRef) -> Result<(), Error>;
}

#[cfg(feature = "reqwest")]
mod reqwest;
