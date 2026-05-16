use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Request {
    Ping,
    Status,
    Reload,
    Sync { repo: Option<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Response {
    Pong,
    Status { repos: Vec<RepoStatus> },
    Reloaded { ok: bool, message: String },
    Synced { queued: Vec<String> },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoStatus {
    pub name: String,
    pub path: String,
}

impl Response {
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip_is_tagged_json() {
        let req = Request::Sync {
            repo: Some("notes".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert_eq!(json, r#"{"kind":"sync","repo":"notes"}"#);
        assert_eq!(serde_json::from_str::<Request>(&json).unwrap(), req);
    }
}
