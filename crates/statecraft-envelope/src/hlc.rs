//! The issuer's hybrid logical clock. Stamped by the issuer, ordered
//! lexicographically, never read as wall time by a verifier.

use serde::{Deserialize, Serialize};

use crate::Error;
use crate::value::Value;

/// `(physical, logical)`: physical is the issuer's millisecond count, logical
/// breaks ties. Ordering is lexicographic.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub struct Hlc {
    /// The issuer's millisecond count.
    pub physical: u64,
    /// The tie-breaker.
    pub logical: u32,
}

impl Hlc {
    /// Build.
    pub fn new(physical: u64, logical: u32) -> Self {
        Hlc { physical, logical }
    }

    /// The next clock strictly after both `self` and `other`.
    pub fn after(self, other: Hlc) -> Hlc {
        let m = self.max(other);
        Hlc {
            physical: m.physical,
            logical: m.logical + 1,
        }
    }

    /// The canonical value form: `{ "logical", "physical" }`.
    pub fn to_value(self) -> Value {
        Value::map()
            .with("physical", Value::Int(self.physical as i64))
            .unwrap()
            .with("logical", Value::Int(self.logical as i64))
            .unwrap()
    }

    /// From the canonical value form.
    pub fn from_value(v: &Value) -> Result<Self, Error> {
        let m = v
            .as_map()
            .ok_or_else(|| Error::Validation("hlc is not a map".into()))?;
        let get = |k: &str| -> Result<i64, Error> {
            match m.get(k) {
                Some(Value::Int(i)) if *i >= 0 => Ok(*i),
                _ => Err(Error::Validation(format!("hlc.{k} missing or negative"))),
            }
        };
        let logical = get("logical")?;
        if logical > u32::MAX as i64 {
            return Err(Error::Validation("hlc.logical too large".into()));
        }
        Ok(Hlc {
            physical: get("physical")? as u64,
            logical: logical as u32,
        })
    }
}
