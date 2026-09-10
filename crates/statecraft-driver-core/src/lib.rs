//! The provider-neutral half of a Statecraft driver (spec 114).
//!
//! Spec 014's session driver with the provider removed: read the 043
//! request, spawn whatever the [`Provider`] names with argv only, deliver
//! the prompt on stdin, stream the child's stdout through the provider's
//! parser, enforce the deadline and the shutdown kill, classify the
//! termination through the provider's rule table, and emit the `journal`,
//! `stream` and `result` events the engine consumes. Nothing in this crate
//! names a provider.

pub mod classify;
pub mod protocol;
pub mod session;

use std::collections::BTreeMap;

use serde_json::Value;

pub use classify::{Rule, TerminationKind};
pub use statecraft_contract::{CapabilityTier, ModelTier, SessionRequest, SessionResult};

/// What the core learned from one line of the provider's stdout.
#[derive(Clone, Debug, PartialEq)]
pub enum ProviderEvent {
    /// The provider announced its session id.
    Init { session_id: Option<String> },
    /// The provider's final report.
    Result(ResultEvent),
    /// Anything else the provider said, kept verbatim for the sink.
    Other,
}

/// The subset of a provider's result event the core reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResultEvent {
    pub is_error: bool,
    pub subtype: Option<String>,
    pub result_text: Option<String>,
    pub session_id: Option<String>,
    pub total_cost_usd: Option<f64>,
    pub usage: Option<Value>,
    pub num_turns: Option<u64>,
}

/// The execution posture a session runs under (032), as the request carries
/// it: the provider turns it into flags.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    pub mode: String,
    pub allowed_tools: Option<Vec<String>>,
    pub disallowed_tools: Option<Vec<String>>,
    pub models: Option<(String, String)>,
    /// The driver the project's sessions run on (spec 117 B-5), carried so
    /// the journal a Rust driver writes matches the TypeScript one's.
    pub driver: Option<String>,
}

impl Profile {
    /// 032 B-5's payload: mode, both lists and the pair, explicit nulls.
    pub fn payload(&self) -> Value {
        serde_json::json!({
            "mode": self.mode,
            "allowedTools": self.allowed_tools,
            "disallowedTools": self.disallowed_tools,
            "models": self.models.as_ref().map(|(s, f)| serde_json::json!({"strong": s, "fast": f})),
            "driver": self.driver,
        })
    }

    pub fn from_payload(value: &Value) -> Result<Profile, String> {
        let obj = value.as_object().ok_or("profile: expected a JSON object")?;
        let mode = obj
            .get("mode")
            .and_then(Value::as_str)
            .filter(|m| *m == "bypass" || *m == "guarded")
            .ok_or("profile: expected mode \"bypass\" or \"guarded\"")?
            .to_string();
        let list = |key: &str| -> Result<Option<Vec<String>>, String> {
            match obj.get(key) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|i| {
                        i.as_str()
                            .map(String::from)
                            .ok_or_else(|| format!("profile: {key} must be strings"))
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Some),
                Some(_) => Err(format!("profile: {key} must be an array")),
            }
        };
        let models = match obj.get("models") {
            None | Some(Value::Null) => None,
            Some(m) => {
                let strong = m
                    .get("strong")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty());
                let fast = m
                    .get("fast")
                    .and_then(Value::as_str)
                    .filter(|s| !s.trim().is_empty());
                match (strong, fast) {
                    (Some(s), Some(f)) => Some((s.to_string(), f.to_string())),
                    _ => {
                        return Err(
                            "profile: models must carry a non-empty strong and fast pair"
                                .to_string(),
                        )
                    }
                }
            }
        };
        let driver = match obj.get("driver") {
            None | Some(Value::Null) => None,
            Some(Value::String(name)) if !name.trim().is_empty() => Some(name.clone()),
            Some(_) => return Err("profile: driver must be a name or null".to_string()),
        };
        Ok(Profile {
            mode,
            allowed_tools: list("allowedTools")?,
            disallowed_tools: list("disallowedTools")?,
            models,
            driver,
        })
    }
}

/// What the core hands a provider to build its argv.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpawnSpec<'a> {
    pub profile: &'a Profile,
    pub model: Option<&'a str>,
    pub max_turns: Option<u64>,
    pub mcp_config_path: Option<&'a str>,
}

/// The thin crate's contract (spec 114 B-1).
pub trait Provider: Send + Sync {
    /// The member's dispatch name, for the manifest and diagnostics.
    fn name(&self) -> &'static str;
    /// The provider binary, and the env var that overrides it.
    fn default_bin(&self) -> &'static str;
    fn bin_env_var(&self) -> &'static str;
    /// The argv after the binary. The prompt is never here (014 B-1).
    fn argv(&self, spec: &SpawnSpec<'_>) -> Vec<String>;
    /// The child's environment from the parent's.
    fn child_env(&self, parent: &BTreeMap<String, String>) -> BTreeMap<String, String>;
    /// One parsed stdout line.
    fn parse_event(&self, event: &Value) -> ProviderEvent;
    /// Where the provider writes its own transcript for a session id.
    fn transcript_path(&self, repo: &str, session_id: &str) -> Option<String>;
    /// The default model pair (040 B-3): `(strong, fast)`.
    fn default_models(&self) -> (&'static str, &'static str);
    /// 014 B-4's rule table, in priority order.
    fn termination_rules(&self) -> &[Rule];
    /// The provider's own fields in `session.init` (its binary under its own key,
    /// D-2), given the resolved binary.
    fn init_extras(&self, bin: &str) -> Value;
    /// The result subtype that means the turn cap was hit, when the provider
    /// has a turn cap (spec 116 B-1). None: `max-turns` is never classified.
    fn max_turns_subtype(&self) -> Option<&'static str> {
        None
    }
    /// The tier the manifest declares (spec 116 B-1, 042 B-3).
    fn capability_tier(&self) -> CapabilityTier {
        CapabilityTier::Reference
    }
    /// What the request made the provider give up, as fields merged into
    /// `session.init` beside `init_extras` (spec 116 B-1, doc 01 D22).
    fn spawn_extras(&self, _spec: &SpawnSpec<'_>) -> Value {
        Value::Object(serde_json::Map::new())
    }
    /// Spec 119 B-5 (doc 04 D41): the text of a refused tool call when this
    /// stdout line is the harness's structured event for one, else None.
    /// Never the model's prose: a quotation of "blocked" is not a denial.
    fn denial_in_event(&self, _event: &Value) -> Option<String> {
        None
    }
    /// Spec 119 D-6: the refusals a harness reports on its own stderr rather
    /// than on the stream (a tool router that logs one line per blocked
    /// command). Read once over the bounded stderr tail at session end.
    fn denials_in_stderr(&self, _stderr_tail: &str) -> Vec<String> {
        Vec::new()
    }
}

/// The model an explicit id, the project's pair or the default pair resolves
/// to for a tier (040 B-1).
pub fn resolve_model(
    provider: &dyn Provider,
    tier: Option<ModelTier>,
    explicit: Option<&str>,
    profile: &Profile,
) -> Option<String> {
    if let Some(m) = explicit {
        return Some(m.to_string());
    }
    let tier = tier?;
    let (strong, fast) = match &profile.models {
        Some((s, f)) => (s.as_str(), f.as_str()),
        None => provider.default_models(),
    };
    Some(match tier {
        ModelTier::Strong => strong.to_string(),
        ModelTier::Fast => fast.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_round_trips_its_payload() {
        let p = Profile {
            mode: "guarded".into(),
            allowed_tools: Some(vec!["Read".into()]),
            disallowed_tools: None,
            models: Some(("s".into(), "f".into())),
            driver: Some("other".into()),
        };
        let back = Profile::from_payload(&p.payload()).unwrap();
        assert_eq!(back, p);
        assert_eq!(p.payload()["driver"], serde_json::json!("other"));
        // Spec 117 B-5: absent and null both read as no driver, and the
        // payload says null out loud.
        let none = Profile::from_payload(&serde_json::json!({"mode": "bypass"})).unwrap();
        assert_eq!(none.driver, None);
        assert_eq!(none.payload()["driver"], Value::Null);
        assert!(
            Profile::from_payload(&serde_json::json!({"mode": "bypass", "driver": 3})).is_err()
        );
        assert!(Profile::from_payload(&serde_json::json!({"mode": "x"})).is_err());
        assert!(Profile::from_payload(
            &serde_json::json!({"mode": "bypass", "models": {"strong": "a"}})
        )
        .is_err());
    }

    #[test]
    fn the_core_names_no_provider() {
        // FR-005, the way spec 112's core checks itself.
        let words = [["cl", "aude"].concat(), ["co", "dex"].concat()];
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(&src).unwrap() {
            let path = entry.unwrap().path();
            let text = std::fs::read_to_string(&path).unwrap().to_lowercase();
            for word in &words {
                assert_eq!(
                    text.matches(word.as_str()).count(),
                    0,
                    "{} names a provider",
                    path.display()
                );
            }
        }
    }
}
