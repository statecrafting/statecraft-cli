//! Section 3.4's prohibited-content screen, step 7 of section 3.6.
//!
//! Walks every decoded member and string value of a manifest before any of
//! it can be retained. Under `metadata-only` a member is prohibited when it
//! is outside the exact v1 member set ([`super::manifest::SCHEMA`]), and a
//! string value is prohibited when the secret detector the attempt's policy
//! names finds a secret in it. A missing detector identity, a named detector
//! that was not supplied, a detector whose identity is not the one recorded,
//! and a scan that cannot complete each refuse rather than retain.
//!
//! A finding carries the member path, the detector identity when one fired,
//! and a category. **It never carries the candidate value**, and a member name
//! the detector flags is replaced in the path by `[secret-name]`, so a path
//! cannot smuggle the value it reports.

use serde::{Deserialize, Serialize};

use super::bounded::Json;
use super::manifest::Shape;

/// The detector this product ships: action-gate-core's default secret rules
/// at the pinned release, which wire-witness spec 003 section 3.2 names for
/// the producer's own redaction, plus exact values the attempt registers.
pub const DEFAULT_DETECTOR: &str =
    "statecraft/secret-detector/v1:action-gate-core@0.3.0+registered";

/// Why a scan could not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanFailure;

/// A secret detector with an exact identity.
pub trait SecretDetector {
    /// The identity a policy records and a finding names.
    fn identity(&self) -> &str;
    /// The id of the rule that found a secret in `text`, if one did.
    ///
    /// # Errors
    ///
    /// [`ScanFailure`] when the text could not be scanned.
    fn scan(&self, text: &str) -> Result<Option<String>, ScanFailure>;
}

/// [`DEFAULT_DETECTOR`]: the default rules, and every registered value as an
/// exact substring. The registered values are held only in memory and never
/// written anywhere.
#[derive(Debug, Clone, Default)]
pub struct DefaultDetector {
    registered: Vec<String>,
}

impl DefaultDetector {
    /// The default rules with these registered values; an empty value is
    /// ignored, because it would match everything.
    pub fn with_registered(values: impl IntoIterator<Item = String>) -> Self {
        DefaultDetector {
            registered: values.into_iter().filter(|v| !v.is_empty()).collect(),
        }
    }
}

impl SecretDetector for DefaultDetector {
    fn identity(&self) -> &str {
        DEFAULT_DETECTOR
    }

    fn scan(&self, text: &str) -> Result<Option<String>, ScanFailure> {
        if self.registered.iter().any(|v| text.contains(v.as_str())) {
            return Ok(Some("registered-secret".to_string()));
        }
        let rules = action_gate_core::secrets::SecretRules::default();
        Ok(action_gate_core::secrets::scan(text, &rules).map(|f| f.detector.as_str().to_string()))
    }
}

/// What a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    /// A member outside the exact v1 member set.
    OutsideSchema,
    /// A string value the detector flagged.
    Secret,
    /// The policy names no detector.
    DetectorMissing,
    /// The policy names a detector, and none was supplied to run.
    DetectorUnavailable,
    /// The detector supplied is not the one the policy names.
    DetectorMismatch,
    /// The detector could not complete a scan.
    ScanIncomplete,
}

/// One content finding: path, detector and category, never the value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// The member path, `$` for the whole manifest.
    pub path: String,
    /// The detector's identity and rule, when one fired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detector: Option<String>,
    /// What was found.
    pub category: Category,
}

/// Screen `manifest` against `schema` with the detector the policy names.
///
/// Returns every finding, in walk order; an empty list is a pass. With no
/// usable detector the walk does not run and the one finding says why.
pub fn screen(
    manifest: &Json,
    schema: &Shape,
    policy_detector: Option<&str>,
    detector: Option<&dyn SecretDetector>,
) -> Vec<Finding> {
    let whole = |category| {
        vec![Finding {
            path: "$".to_string(),
            detector: None,
            category,
        }]
    };
    let Some(expected) = policy_detector else {
        return whole(Category::DetectorMissing);
    };
    let Some(detector) = detector else {
        return whole(Category::DetectorUnavailable);
    };
    if detector.identity() != expected {
        return whole(Category::DetectorMismatch);
    }
    let mut walk = Walk {
        detector,
        findings: Vec::new(),
    };
    walk.value(manifest, Some(schema), "$".to_string());
    walk.findings
}

struct Walk<'d> {
    detector: &'d dyn SecretDetector,
    findings: Vec<Finding>,
}

impl Walk<'_> {
    fn flag(&mut self, path: String, detector: Option<String>, category: Category) {
        self.findings.push(Finding {
            path,
            detector,
            category,
        });
    }

    /// The name as a path segment, or `[secret-name]` when it is one. A name
    /// that cannot be scanned is reported and kept out of the path.
    fn segment(&mut self, parent: &str, name: &str) -> String {
        match self.detector.scan(name) {
            Ok(None) => format!("{parent}.{name}"),
            Ok(Some(rule)) => {
                let path = format!("{parent}.[secret-name]");
                self.flag(path.clone(), Some(self.rule(&rule)), Category::Secret);
                path
            }
            Err(ScanFailure) => {
                let path = format!("{parent}.[unscanned-name]");
                self.flag(path.clone(), None, Category::ScanIncomplete);
                path
            }
        }
    }

    fn rule(&self, rule: &str) -> String {
        format!("{}#{rule}", self.detector.identity())
    }

    /// Walk `value` at `path`. `shape` is `None` below a member already
    /// reported outside the schema: its contents are still scanned, so a
    /// secret inside it is named too, but not reported member by member.
    fn value(&mut self, value: &Json, shape: Option<&Shape>, path: String) {
        match value {
            Json::String(s) => match self.detector.scan(s) {
                Ok(None) => {}
                Ok(Some(rule)) => {
                    let detector = Some(self.rule(&rule));
                    self.flag(path, detector, Category::Secret);
                }
                Err(ScanFailure) => self.flag(path, None, Category::ScanIncomplete),
            },
            Json::Array(items) => {
                let inner = match shape {
                    Some(Shape::Array(inner)) => Some(*inner),
                    _ => None,
                };
                for (i, item) in items.iter().enumerate() {
                    self.value(item, inner, format!("{path}[{i}]"));
                }
            }
            Json::Object(members) => {
                let allowed: Option<&[(&str, Shape)]> = match shape {
                    Some(Shape::Object(m) | Shape::LeafOr(m)) => Some(m),
                    _ => None,
                };
                for (name, member) in members {
                    let at = self.segment(&path, name);
                    let inner = allowed.and_then(|m| m.iter().find(|(n, _)| n == name));
                    if shape.is_some() && inner.is_none() {
                        self.flag(at.clone(), None, Category::OutsideSchema);
                    }
                    self.value(member, inner.map(|(_, s)| s), at);
                }
            }
            Json::Null | Json::Bool(_) | Json::Number(_) => {}
        }
    }
}
