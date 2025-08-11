use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Kind of envelope used in the JSON protocol.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Request,
    Response,
    Event,
}

/// Standard RPC style error object.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

/// Top level envelope exchanged between core and plugins.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Envelope {
    pub id: Option<String>,
    pub kind: Kind,
    pub method: Option<String>,
    pub params: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
    pub topic: Option<String>,
    pub payload: Option<Value>,
}

/// Metadata a plugin provides during the init phase.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub needs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_roundtrip() {
        let env = Envelope {
            id: Some("1".into()),
            kind: Kind::Request,
            method: Some("test".into()),
            params: Some(serde_json::json!({"a":1})),
            result: None,
            error: None,
            topic: None,
            payload: None,
        };
        let s = serde_json::to_string(&env).unwrap();
        let de: Envelope = serde_json::from_str(&s).unwrap();
        assert_eq!(env, de);
    }
}
