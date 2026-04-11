// Copyright 2026 The Orbflow Authors
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Error-to-HTTP-response mapping.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use orbflow_core::OrbflowError;
use serde::Serialize;

/// JSON error body returned in API responses.
/// Matches the frontend's expected envelope: `{ data: null, error: "..." }`.
#[derive(Debug, Serialize)]
struct ErrorBody {
    data: Option<()>,
    error: String,
}

/// Maps a [`OrbflowError`] to an HTTP status code.
pub fn error_to_status(err: &OrbflowError) -> StatusCode {
    if err.is_validation_error() {
        return StatusCode::BAD_REQUEST;
    }
    match err {
        OrbflowError::NotFound | OrbflowError::NodeNotFound => StatusCode::NOT_FOUND,
        OrbflowError::Conflict => StatusCode::CONFLICT,
        OrbflowError::AlreadyExists => StatusCode::CONFLICT,
        OrbflowError::InvalidStatus => StatusCode::UNPROCESSABLE_ENTITY,
        OrbflowError::Cancelled => StatusCode::GONE,
        OrbflowError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        OrbflowError::Forbidden(_) => StatusCode::FORBIDDEN,
        OrbflowError::BudgetExceeded(_) => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Converts a [`OrbflowError`] into an Axum [`Response`] with the appropriate
/// status code and a JSON error body.
///
/// For client errors (4xx), the error message is forwarded to the caller.
/// For server errors (5xx), a generic message is returned to avoid leaking
/// internal details (e.g. Postgres errors, file paths, stack traces).
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = error_to_status(&self.0);
        let message = if status.is_server_error() {
            tracing::error!(error = %self.0, "internal server error");
            "internal server error".to_string()
        } else {
            sanitize_client_error(&self.0)
        };
        let body = ErrorBody {
            data: None,
            error: message,
        };
        (status, Json(body)).into_response()
    }
}

/// Converts a [`OrbflowError`] to a safe HTTP response, redacting internal
/// details for 5xx errors while passing through 4xx error messages.
///
/// Use this instead of `write_error(status, e.to_string())` in handlers.
pub fn write_safe_error(err: &OrbflowError) -> Response {
    let status = error_to_status(err);
    if status.is_server_error() {
        tracing::error!(error = %err, "internal server error");
        write_error(status, "internal server error")
    } else {
        write_error(status, sanitize_client_error(err))
    }
}

/// Sanitizes client-facing error messages to avoid leaking internal details.
///
/// `Forbidden` and `BudgetExceeded` carry free-form strings that may contain
/// internal role names, org IDs, or topology info. We replace them with
/// fixed messages while logging the full detail server-side.
fn sanitize_client_error(err: &OrbflowError) -> String {
    match err {
        OrbflowError::Forbidden(detail) => {
            tracing::warn!(detail = %detail, "forbidden");
            "orbflow: forbidden".to_string()
        }
        OrbflowError::BudgetExceeded(detail) => {
            tracing::warn!(detail = %detail, "budget exceeded");
            "orbflow: budget exceeded".to_string()
        }
        other => other.to_string(),
    }
}

/// Wrapper around [`OrbflowError`] that implements [`IntoResponse`].
pub struct ApiError(pub OrbflowError);

impl From<OrbflowError> for ApiError {
    fn from(err: OrbflowError) -> Self {
        Self(err)
    }
}

/// Writes a JSON error response with the given status code and message.
pub fn write_error(status: StatusCode, message: impl Into<String>) -> Response {
    let body = ErrorBody {
        data: None,
        error: message.into(),
    };
    (status, Json(body)).into_response()
}

/// JSON data envelope for single-item responses.
#[derive(Debug, Serialize)]
pub struct DataResponse<T: Serialize> {
    pub data: T,
}

/// JSON envelope for paginated list responses.
#[derive(Debug, Serialize)]
pub struct ListResponse<T: Serialize> {
    pub data: T,
    pub meta: ListMeta,
}

/// Pagination metadata.
#[derive(Debug, Serialize)]
pub struct ListMeta {
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
}

/// Builds a single-item JSON response with the given status code.
pub fn write_data<T: Serialize>(status: StatusCode, data: T) -> Response {
    (status, Json(DataResponse { data })).into_response()
}

/// Builds a paginated list JSON response.
pub fn write_list<T: Serialize>(data: T, total: i64, offset: i64, limit: i64) -> Response {
    let body = ListResponse {
        data,
        meta: ListMeta {
            total,
            offset,
            limit,
        },
    };
    (StatusCode::OK, Json(body)).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use serde_json::Value;

    /// Helper: extract status code and parsed JSON body from a Response.
    async fn decompose(resp: Response) -> (StatusCode, Value) {
        let status = resp.status();
        let bytes = to_bytes(resp.into_body(), 1024 * 64).await.unwrap();
        let json: Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    // ── error_to_status: validation errors → 400 ──────────────────────

    #[test]
    fn maps_cycle_detected_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::CycleDetected),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_duplicate_node_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::DuplicateNode),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_duplicate_edge_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::DuplicateEdge),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_invalid_edge_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::InvalidEdge),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_no_entry_nodes_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::NoEntryNodes),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_disconnected_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::Disconnected),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_invalid_node_config_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::InvalidNodeConfig("bad".into())),
            StatusCode::BAD_REQUEST,
        );
    }

    #[test]
    fn maps_invalid_node_kind_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::InvalidNodeKind),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_missing_capability_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::MissingCapability),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_invalid_capability_edge_to_bad_request() {
        assert_eq!(
            error_to_status(&OrbflowError::InvalidCapabilityEdge),
            StatusCode::BAD_REQUEST
        );
    }

    // ── error_to_status: non-validation variants ──────────────────────

    #[test]
    fn maps_not_found_to_404() {
        assert_eq!(
            error_to_status(&OrbflowError::NotFound),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn maps_node_not_found_to_404() {
        assert_eq!(
            error_to_status(&OrbflowError::NodeNotFound),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn maps_conflict_to_409() {
        assert_eq!(
            error_to_status(&OrbflowError::Conflict),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn maps_already_exists_to_409() {
        assert_eq!(
            error_to_status(&OrbflowError::AlreadyExists),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn maps_invalid_status_to_422() {
        assert_eq!(
            error_to_status(&OrbflowError::InvalidStatus),
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[test]
    fn maps_cancelled_to_410() {
        assert_eq!(error_to_status(&OrbflowError::Cancelled), StatusCode::GONE);
    }

    #[test]
    fn maps_timeout_to_504() {
        assert_eq!(
            error_to_status(&OrbflowError::Timeout),
            StatusCode::GATEWAY_TIMEOUT
        );
    }

    #[test]
    fn maps_forbidden_to_403() {
        assert_eq!(
            error_to_status(&OrbflowError::Forbidden("nope".into())),
            StatusCode::FORBIDDEN,
        );
    }

    #[test]
    fn maps_budget_exceeded_to_429() {
        assert_eq!(
            error_to_status(&OrbflowError::BudgetExceeded("over limit".into())),
            StatusCode::TOO_MANY_REQUESTS,
        );
    }

    #[test]
    fn maps_internal_to_500() {
        assert_eq!(
            error_to_status(&OrbflowError::Internal("oops".into())),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    #[test]
    fn maps_database_to_500() {
        assert_eq!(
            error_to_status(&OrbflowError::Database("pg gone".into())),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    #[test]
    fn maps_bus_to_500() {
        assert_eq!(
            error_to_status(&OrbflowError::Bus("nats down".into())),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    #[test]
    fn maps_crypto_to_500() {
        assert_eq!(
            error_to_status(&OrbflowError::Crypto("decrypt failed".into())),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    #[test]
    fn maps_engine_stopped_to_500() {
        assert_eq!(
            error_to_status(&OrbflowError::EngineStopped),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    // ── write_safe_error: 4xx preserves message, 5xx redacts ──────────

    #[tokio::test]
    async fn safe_error_preserves_4xx_message() {
        let err = OrbflowError::NotFound;
        let (status, body) = decompose(write_safe_error(&err)).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body["data"].is_null());
        assert_eq!(body["error"].as_str().unwrap(), "orbflow: not found");
    }

    #[tokio::test]
    async fn safe_error_preserves_validation_message() {
        let err = OrbflowError::InvalidNodeConfig("field X is required".into());
        let (status, body) = decompose(write_safe_error(&err)).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("field X is required")
        );
    }

    #[tokio::test]
    async fn safe_error_redacts_5xx_message() {
        let err = OrbflowError::Database("connection refused at 10.0.0.5:5432".into());
        let (status, body) = decompose(write_safe_error(&err)).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(body["data"].is_null());
        assert_eq!(body["error"].as_str().unwrap(), "internal server error");
    }

    #[tokio::test]
    async fn safe_error_redacts_internal_error() {
        let err = OrbflowError::Internal("stack trace here".into());
        let (status, body) = decompose(write_safe_error(&err)).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"].as_str().unwrap(), "internal server error");
    }

    // ── write_error: JSON envelope format ─────────────────────────────

    #[tokio::test]
    async fn write_error_returns_correct_envelope() {
        let (status, body) = decompose(write_error(StatusCode::BAD_REQUEST, "bad input")).await;

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["data"].is_null());
        assert_eq!(body["error"].as_str().unwrap(), "bad input");
    }

    #[tokio::test]
    async fn write_error_accepts_string_owned() {
        let msg = String::from("owned message");
        let (_, body) = decompose(write_error(StatusCode::FORBIDDEN, msg)).await;

        assert_eq!(body["error"].as_str().unwrap(), "owned message");
    }

    // ── write_data: JSON success envelope ─────────────────────────────

    #[tokio::test]
    async fn write_data_returns_correct_envelope() {
        let (status, body) = decompose(write_data(StatusCode::OK, "hello")).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"].as_str().unwrap(), "hello");
        assert!(body.get("error").is_none());
    }

    #[tokio::test]
    async fn write_data_with_created_status() {
        let (status, body) = decompose(write_data(StatusCode::CREATED, 42)).await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["data"].as_i64().unwrap(), 42);
    }

    #[tokio::test]
    async fn write_data_with_struct() {
        #[derive(Serialize)]
        struct Item {
            id: u64,
            name: String,
        }

        let item = Item {
            id: 1,
            name: "widget".into(),
        };
        let (status, body) = decompose(write_data(StatusCode::OK, item)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"]["id"].as_u64().unwrap(), 1);
        assert_eq!(body["data"]["name"].as_str().unwrap(), "widget");
    }

    // ── write_list: paginated envelope ────────────────────────────────

    #[tokio::test]
    async fn write_list_returns_correct_envelope() {
        let items = vec!["a", "b", "c"];
        let (status, body) = decompose(write_list(items, 10, 0, 3)).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"].as_array().unwrap().len(), 3);
        assert_eq!(body["meta"]["total"].as_i64().unwrap(), 10);
        assert_eq!(body["meta"]["offset"].as_i64().unwrap(), 0);
        assert_eq!(body["meta"]["limit"].as_i64().unwrap(), 3);
    }

    // ── ApiError: IntoResponse impl ───────────────────────────────────

    #[tokio::test]
    async fn api_error_4xx_preserves_message() {
        let err = ApiError(OrbflowError::AlreadyExists);
        let (status, body) = decompose(err.into_response()).await;

        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"].as_str().unwrap(), "orbflow: already exists");
        assert!(body["data"].is_null());
    }

    #[tokio::test]
    async fn api_error_5xx_redacts_message() {
        let err = ApiError(OrbflowError::Bus("nats connection lost".into()));
        let (status, body) = decompose(err.into_response()).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["error"].as_str().unwrap(), "internal server error");
    }

    #[tokio::test]
    async fn api_error_from_orbflow_error() {
        let orbflow_err = OrbflowError::Conflict;
        let api_err: ApiError = orbflow_err.into();
        let (status, _) = decompose(api_err.into_response()).await;

        assert_eq!(status, StatusCode::CONFLICT);
    }

    // ── Forbidden/BudgetExceeded sanitization ────────────────────────

    #[tokio::test]
    async fn forbidden_error_sanitizes_detail() {
        let err = ApiError(OrbflowError::Forbidden(
            "user X at IP 10.0.0.5 denied by rule org_admin".into(),
        ));
        let (status, body) = decompose(err.into_response()).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        // Must NOT leak internal detail
        assert_eq!(body["error"].as_str().unwrap(), "orbflow: forbidden");
        assert!(!body["error"].as_str().unwrap().contains("10.0.0.5"));
    }

    #[tokio::test]
    async fn budget_exceeded_error_sanitizes_detail() {
        let err = ApiError(OrbflowError::BudgetExceeded(
            "org-id: foo consumed 1000/1000 tokens".into(),
        ));
        let (status, body) = decompose(err.into_response()).await;

        assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(body["error"].as_str().unwrap(), "orbflow: budget exceeded");
        assert!(!body["error"].as_str().unwrap().contains("foo"));
    }

    #[tokio::test]
    async fn safe_error_sanitizes_forbidden() {
        let err = OrbflowError::Forbidden("admin only: role required: org_admin".into());
        let (status, body) = decompose(write_safe_error(&err)).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"].as_str().unwrap(), "orbflow: forbidden");
    }
}
