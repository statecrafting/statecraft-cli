//! The in-memory value model every hashed byte passes through (hqgit 011).
//!
//! No floats, no indefinite items, map keys are text, integers are the
//! portable range only. A [`Value`] is what the canonical encoder consumes
//! and what claims and fact bodies are built from.

use std::collections::BTreeMap;

use crate::Error;
use crate::hash::Cid;

/// The largest integer magnitude a portable reader must accept (PKG-04).
pub const PORTABLE_MAX: i64 = (1 << 53) - 1;

/// A value in the canonical model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// `null`.
    Null,
    /// A boolean.
    Bool(bool),
    /// An integer inside the portable range.
    Int(i64),
    /// A byte string.
    Bytes(Vec<u8>),
    /// A text string.
    Text(String),
    /// An array.
    Array(Vec<Value>),
    /// A map with text keys. `BTreeMap` so iteration is never ambient.
    Map(BTreeMap<String, Value>),
    /// A link to a content-addressed object (the DAG-CBOR link tag).
    Link(Cid),
}

impl Value {
    /// An integer, refused outside the portable range.
    pub fn int(i: i64) -> Result<Self, Error> {
        if !(-PORTABLE_MAX..=PORTABLE_MAX).contains(&i) {
            return Err(Error::Validation(format!(
                "integer {i} outside the portable range"
            )));
        }
        Ok(Value::Int(i))
    }

    /// Text.
    pub fn text(s: impl Into<String>) -> Self {
        Value::Text(s.into())
    }

    /// Bytes.
    pub fn bytes(b: impl Into<Vec<u8>>) -> Self {
        Value::Bytes(b.into())
    }

    /// An empty map.
    pub fn map() -> Self {
        Value::Map(BTreeMap::new())
    }

    /// Insert into a map value; an error on a non-map.
    pub fn with(mut self, key: &str, v: Value) -> Result<Self, Error> {
        match &mut self {
            Value::Map(m) => {
                m.insert(key.to_string(), v);
                Ok(self)
            }
            _ => Err(Error::Validation("with() on a non-map".into())),
        }
    }

    /// The map inside, if this is a map.
    pub fn as_map(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    /// The text inside, if this is text.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(t) => Some(t),
            _ => None,
        }
    }

    /// The array inside, if this is an array.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// Convert from JSON. Floats and non-portable integers are refused, so a
    /// claim that came in as JSON never carries a value the model cannot hash.
    /// JSON has no bytes or links: hashes travel as hex text in claims.
    pub fn from_json(j: &serde_json::Value) -> Result<Self, Error> {
        Ok(match j {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => match n.as_i64() {
                Some(i) => Value::int(i)?,
                None => {
                    return Err(Error::Validation(format!(
                        "number {n} is not a portable integer"
                    )));
                }
            },
            serde_json::Value::String(s) => Value::Text(s.clone()),
            serde_json::Value::Array(a) => {
                Value::Array(a.iter().map(Value::from_json).collect::<Result<_, _>>()?)
            }
            serde_json::Value::Object(o) => {
                let mut m = BTreeMap::new();
                for (k, v) in o {
                    m.insert(k.clone(), Value::from_json(v)?);
                }
                Value::Map(m)
            }
        })
    }

    /// Convert to JSON. Bytes become lowercase hex text and links become
    /// `{ "/": { "codec", "hash" } }` maps, both of which are stable and
    /// readable; this is a rendering, never a hashed form.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(i) => serde_json::Value::from(*i),
            Value::Bytes(b) => serde_json::Value::String(hex::encode(b)),
            Value::Text(t) => serde_json::Value::String(t.clone()),
            Value::Array(a) => serde_json::Value::Array(a.iter().map(Value::to_json).collect()),
            Value::Map(m) => {
                serde_json::Value::Object(m.iter().map(|(k, v)| (k.clone(), v.to_json())).collect())
            }
            Value::Link(c) => {
                serde_json::json!({ "/": { "codec": c.codec.as_str(), "hash": c.hash.to_hex() } })
            }
        }
    }
}
