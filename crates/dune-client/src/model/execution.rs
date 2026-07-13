use std::{collections::BTreeMap, fmt};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Serializer, de::DeserializeOwned};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use strum::{Display, EnumDiscriminants, EnumIter, IntoEnumIterator};

/// Identifier of a saved Dune query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QueryId(pub i64);

impl fmt::Display for QueryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Identifier of a single execution of a Dune query.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExecutionId(pub String);

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for ExecutionId {
    fn from(id: &str) -> Self {
        Self(id.to_owned())
    }
}

/// A typed parameter passed to a Dune query execution.
///
/// The variant encodes the Dune parameter type, replacing the stringly-typed
/// `type` field used by the HTTP API. All variants serialize to the string
/// representation Dune expects in the `query_parameters` map.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterValue {
    Text(String),
    Number(f64),
    /// Rendered in the `YYYY-MM-DD HH:MM:SS` (UTC) format Dune expects at
    /// request time.
    Date(DateTime<Utc>),
    Enum(String),
}

/// The date format Dune expects for `date` query parameters.
const DUNE_DATE_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

impl fmt::Display for ParameterValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(s) | Self::Enum(s) => s.fmt(f),
            Self::Number(n) => n.fmt(f),
            Self::Date(d) => d.format(DUNE_DATE_FORMAT).fmt(f),
        }
    }
}

impl Serialize for ParameterValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// A named parameter for a query execution.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryParameter {
    pub key: String,
    pub value: ParameterValue,
}

impl QueryParameter {
    pub fn new(key: impl Into<String>, value: ParameterValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }
}

/// Request body for `POST /query/{id}/execute`.
#[derive(Debug, Serialize)]
pub struct ExecuteRequest<'a> {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub query_parameters: BTreeMap<&'a str, &'a ParameterValue>,
}

impl<'a> From<&'a [QueryParameter]> for ExecuteRequest<'a> {
    fn from(params: &'a [QueryParameter]) -> Self {
        Self {
            query_parameters: params.iter().map(|p| (p.key.as_str(), &p.value)).collect(),
        }
    }
}

/// Response of `POST /query/{id}/execute`.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteResponse {
    pub execution_id: ExecutionId,
    pub state: ExecutionState,
}

/// Response of `GET /execution/{id}/results`, generic over the row type.
///
/// Use [`serde_json::Value`] as `T` for untyped rows.
#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "T: DeserializeOwned")]
pub struct ResultsResponse<T> {
    pub execution_id: ExecutionId,
    #[serde(flatten)]
    pub status: ExecutionStatus<T>,
}

/// Response of `GET /query/{id}/results` ("latest results"), generic over the
/// row type. Unlike [`ResultsResponse`] it carries data-freshness and
/// pagination metadata.
#[derive(Debug, Clone, Deserialize)]
#[serde(bound = "T: DeserializeOwned")]
pub struct LatestResultsResponse<T> {
    pub execution_id: ExecutionId,
    /// When the served result's execution finished on Dune — the true
    /// data-freshness signal for consumers that never trigger executions.
    #[serde(default)]
    pub execution_ended_at: Option<DateTime<Utc>>,
    /// Offset to pass to the next page request, when more rows exist.
    #[serde(default)]
    pub next_offset: Option<u64>,
    #[serde(flatten)]
    pub status: ExecutionStatus<T>,
}

/// Execution status tagged by the `state` field. The result payload only
/// exists on the [`ExecutionStatus::Completed`] variant, so a completed
/// execution always carries its rows — no optional fields to check.
///
/// [`ExecutionState`] is derived from this enum's discriminants, keeping the
/// two in sync by construction.
#[derive(Debug, Clone, Deserialize, EnumDiscriminants)]
#[serde(tag = "state", bound = "T: DeserializeOwned")]
#[strum_discriminants(
    name(ExecutionState),
    derive(Hash, Display, EnumIter, SerializeDisplay, DeserializeFromStr),
    strum(serialize_all = "SCREAMING_SNAKE_CASE", prefix = "QUERY_STATE_"),
    doc = "The lifecycle state of a query execution."
)]
pub enum ExecutionStatus<T> {
    #[serde(rename = "QUERY_STATE_PENDING")]
    Pending,
    #[serde(rename = "QUERY_STATE_EXECUTING")]
    Executing,
    #[serde(rename = "QUERY_STATE_COMPLETED")]
    Completed { result: QueryResult<T> },
    #[serde(rename = "QUERY_STATE_FAILED")]
    Failed,
    #[serde(rename = "QUERY_STATE_CANCELLED")]
    Cancelled,
    #[serde(rename = "QUERY_STATE_EXPIRED")]
    Expired,
}

impl<T> ExecutionStatus<T> {
    pub fn state(&self) -> ExecutionState {
        self.into()
    }
}

/// Manual impl because strum's `prefix` attribute applies to `Display` but
/// not to the `FromStr` that `EnumString` would derive.
impl std::str::FromStr for ExecutionState {
    type Err = strum::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::iter()
            .find(|state| state.to_string() == s)
            .ok_or(strum::ParseError::VariantNotFound)
    }
}

/// Rows and metadata from a completed execution.
#[derive(Debug, Clone, Deserialize)]
pub struct QueryResult<T> {
    pub rows: Vec<T>,
    pub metadata: ResultMetadata,
}

/// Metadata describing the result columns.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ResultMetadata {
    pub column_names: Vec<String>,
    pub row_count: u64,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn execute_request_serializes_parameters_as_string_map() {
        let from = "2024-01-01T00:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("valid datetime");
        let params = [
            QueryParameter::new("name", ParameterValue::Text("alice".into())),
            QueryParameter::new("limit", ParameterValue::Number(10.0)),
            QueryParameter::new("ratio", ParameterValue::Number(0.5)),
            QueryParameter::new("from", ParameterValue::Date(from)),
            QueryParameter::new("chain", ParameterValue::Enum("ethereum".into())),
        ];

        let body = serde_json::to_value(ExecuteRequest::from(params.as_slice())).expect("json");
        assert_eq!(
            body,
            json!({
                "query_parameters": {
                    "name": "alice",
                    "limit": "10",
                    "ratio": "0.5",
                    "from": "2024-01-01 00:00:00",
                    "chain": "ethereum"
                }
            })
        );
    }

    #[test]
    fn results_response_completed_carries_result() {
        let response: ResultsResponse<serde_json::Value> = serde_json::from_value(json!({
            "execution_id": "exec-1",
            "state": "QUERY_STATE_COMPLETED",
            "result": {
                "rows": [{ "x": 1 }],
                "metadata": { "column_names": ["x"], "row_count": 1 }
            }
        }))
        .expect("deserializes");

        let ExecutionStatus::Completed { result } = response.status else {
            panic!("expected completed status");
        };
        assert_eq!(result.rows, vec![json!({ "x": 1 })]);
        assert_eq!(result.metadata.row_count, 1);
    }

    #[test]
    fn execution_state_round_trips_through_dune_strings() {
        for (state, repr) in [
            (ExecutionState::Pending, "QUERY_STATE_PENDING"),
            (ExecutionState::Executing, "QUERY_STATE_EXECUTING"),
            (ExecutionState::Completed, "QUERY_STATE_COMPLETED"),
            (ExecutionState::Failed, "QUERY_STATE_FAILED"),
            (ExecutionState::Cancelled, "QUERY_STATE_CANCELLED"),
            (ExecutionState::Expired, "QUERY_STATE_EXPIRED"),
        ] {
            assert_eq!(state.to_string(), repr);
            assert_eq!(
                serde_json::to_value(state).expect("serializes"),
                json!(repr)
            );
            let parsed: ExecutionState = serde_json::from_value(json!(repr)).expect("deserializes");
            assert_eq!(parsed, state);
        }
    }

    #[test]
    fn latest_results_response_deserializes_completed_with_pagination() {
        let response: LatestResultsResponse<serde_json::Value> = serde_json::from_value(json!({
            "execution_id": "exec-1",
            "query_id": 7695360,
            "is_execution_finished": true,
            "state": "QUERY_STATE_COMPLETED",
            "execution_ended_at": "2026-06-10T12:00:00Z",
            "next_offset": 500,
            "next_uri": "https://api.dune.com/api/v1/query/7695360/results?offset=500",
            "result": {
                "rows": [{ "x": 1 }],
                "metadata": { "column_names": ["x"], "row_count": 1 }
            }
        }))
        .expect("deserializes");

        assert_eq!(response.next_offset, Some(500));
        assert_eq!(
            response.execution_ended_at,
            Some("2026-06-10T12:00:00Z".parse::<DateTime<Utc>>().expect("ts"))
        );
        let ExecutionStatus::Completed { result } = response.status else {
            panic!("expected completed");
        };
        assert_eq!(result.rows, vec![json!({ "x": 1 })]);
    }

    #[test]
    fn latest_results_response_tolerates_missing_optional_fields() {
        let response: LatestResultsResponse<serde_json::Value> = serde_json::from_value(json!({
            "execution_id": "exec-2",
            "state": "QUERY_STATE_EXECUTING"
        }))
        .expect("deserializes");

        assert_eq!(response.status.state(), ExecutionState::Executing);
        assert_eq!(response.next_offset, None);
        assert_eq!(response.execution_ended_at, None);
    }

    #[test]
    fn results_response_pending_has_no_result() {
        let response: ResultsResponse<serde_json::Value> = serde_json::from_value(json!({
            "execution_id": "exec-1",
            "state": "QUERY_STATE_PENDING"
        }))
        .expect("deserializes");

        assert_eq!(response.status.state(), ExecutionState::Pending);
    }
}
