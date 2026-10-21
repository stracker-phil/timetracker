use serde::{Serialize, Serializer};
use ts_rs::TS;

pub type AppResult<T> = Result<T, AppError>;

/// Every error that crosses the Tauri boundary. Serialized as `{ kind, message, code, field?, details? }`
/// so the UI can branch on `kind`.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum AppError {
    #[error("{message}")]
    Db {
        message: String,
        code: Option<String>,
    },
    #[error("File error: {0}")]
    Io(String),
    #[error("{0}")]
    InvalidInput(String),
    /// Invalid input in one form field; `field` is its path (`tracker.idleMinutes`).
    #[error("`{field}`: {message}")]
    InvalidField { field: String, message: String },
    #[error("{0}")]
    NotFound(String),
    /// A recording would overlap others; `details` lists them for the UI to offer a replace.
    #[error("{message}")]
    Overlap {
        message: String,
        details: serde_json::Value,
    },
    #[error("{0}")]
    PermissionRequired(String),
    #[error("{0}")]
    TrackerPaused(String),
    #[error("Unknown session {0}")]
    SessionNotFound(String),
    #[error("Cancelled")]
    Cancelled,
}

impl AppError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            AppError::Db { .. } => ErrorKind::Db,
            AppError::Io(_) => ErrorKind::Io,
            AppError::InvalidInput(_) | AppError::InvalidField { .. } => ErrorKind::InvalidInput,
            AppError::NotFound(_) => ErrorKind::NotFound,
            AppError::Overlap { .. } => ErrorKind::Overlap,
            AppError::PermissionRequired(_) => ErrorKind::PermissionRequired,
            AppError::TrackerPaused(_) => ErrorKind::TrackerPaused,
            AppError::SessionNotFound(_) => ErrorKind::SessionNotFound,
            AppError::Cancelled => ErrorKind::Cancelled,
        }
    }

    pub fn code(&self) -> Option<&str> {
        match self {
            AppError::Db { code, .. } => code.as_deref(),
            _ => None,
        }
    }

    pub fn field(&self) -> Option<FieldJson<'_>> {
        match self {
            AppError::InvalidField { field, message } => Some(FieldJson {
                path: field,
                message,
            }),
            _ => None,
        }
    }

    pub fn details(&self) -> Option<&serde_json::Value> {
        match self {
            AppError::Overlap { details, .. } => Some(details),
            _ => None,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ErrorJson {
            kind: self.kind(),
            message: self.to_string(),
            code: self.code(),
            field: self.field(),
            details: self.details(),
        }
        .serialize(serializer)
    }
}

/// The JSON shape of an `AppError`.
#[derive(Serialize, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "AppError")]
struct ErrorJson<'a> {
    kind: ErrorKind,
    message: String,
    code: Option<&'a str>,
    /// The form field the error is about (`InvalidField` only).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    field: Option<FieldJson<'a>>,
    /// What an `overlap` error collides with.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "unknown")]
    details: Option<&'a serde_json::Value>,
}

/// `message` is the error's message without the `` `path`: `` prefix.
#[derive(Serialize, TS, Debug, PartialEq, Eq)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "AppErrorField")]
pub struct FieldJson<'a> {
    pub path: &'a str,
    pub message: &'a str,
}

#[derive(Serialize, TS, Debug, Clone, Copy, PartialEq, Eq)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Db,
    Io,
    InvalidInput,
    NotFound,
    Overlap,
    PermissionRequired,
    TrackerPaused,
    SessionNotFound,
    Cancelled,
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::InvalidInput(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_kind_message_and_code() {
        let err = AppError::Db {
            message: "no such table: recordings".into(),
            code: Some("1".into()),
        };
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({ "kind": "db", "message": "no such table: recordings", "code": "1" })
        );
    }

    #[test]
    fn errors_without_a_code_serialize_it_as_null() {
        assert_eq!(
            serde_json::to_value(AppError::NotFound("Project 3".into())).unwrap(),
            json!({ "kind": "not_found", "message": "Project 3", "code": null })
        );
    }

    #[test]
    fn invalid_field_serializes_its_path_and_message_without_prefix() {
        let err = AppError::InvalidField {
            field: "tracker.idleMinutes".into(),
            message: "must be positive".into(),
        };
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({
                "kind": "invalid_input",
                "message": "`tracker.idleMinutes`: must be positive",
                "code": null,
                "field": { "path": "tracker.idleMinutes", "message": "must be positive" }
            })
        );
    }

    #[test]
    fn overlap_serializes_its_details() {
        let err = AppError::Overlap {
            message: "Overlaps 1 recording".into(),
            details: json!([{ "id": 7 }]),
        };
        assert_eq!(
            serde_json::to_value(&err).unwrap(),
            json!({
                "kind": "overlap",
                "message": "Overlaps 1 recording",
                "code": null,
                "details": [{ "id": 7 }]
            })
        );
    }

    #[test]
    fn field_and_details_are_absent_for_other_errors() {
        let value = serde_json::to_value(AppError::Cancelled).unwrap();
        assert_eq!(
            value,
            json!({ "kind": "cancelled", "message": "Cancelled", "code": null })
        );
    }

    #[test]
    fn every_variant_has_its_kind() {
        let cases = [
            (
                AppError::Db {
                    message: String::new(),
                    code: None,
                },
                ErrorKind::Db,
            ),
            (AppError::Io(String::new()), ErrorKind::Io),
            (
                AppError::InvalidInput(String::new()),
                ErrorKind::InvalidInput,
            ),
            (
                AppError::InvalidField {
                    field: "a".into(),
                    message: "b".into(),
                },
                ErrorKind::InvalidInput,
            ),
            (AppError::NotFound(String::new()), ErrorKind::NotFound),
            (
                AppError::Overlap {
                    message: String::new(),
                    details: json!(null),
                },
                ErrorKind::Overlap,
            ),
            (
                AppError::PermissionRequired(String::new()),
                ErrorKind::PermissionRequired,
            ),
            (
                AppError::TrackerPaused(String::new()),
                ErrorKind::TrackerPaused,
            ),
            (
                AppError::SessionNotFound("s1".into()),
                ErrorKind::SessionNotFound,
            ),
            (AppError::Cancelled, ErrorKind::Cancelled),
        ];
        for (err, kind) in cases {
            assert_eq!(err.kind(), kind, "{err:?}");
        }
    }

    #[test]
    fn io_errors_convert_to_io_with_a_file_error_message() {
        let err = AppError::from(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "denied",
        ));
        assert_eq!(err, AppError::Io("denied".into()));
        assert_eq!(err.to_string(), "File error: denied");
    }

    #[test]
    fn json_errors_convert_to_invalid_input() {
        let parse_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let message = parse_error.to_string();
        let err = AppError::from(parse_error);
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(err.to_string(), message);
    }
}
