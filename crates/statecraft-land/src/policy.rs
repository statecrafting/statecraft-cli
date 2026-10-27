//! Landing values with their configuration provenance (spec 063 section 3.1).
use serde::Serialize;
use serde_json::Value;
use statecraft_environment::manifest::{Manifest, Project};
use statecraft_home::authority::{self, Layer};
use std::{collections::BTreeMap, path::Path, process::Command};

/// One supplied value, never an implicit default.
#[derive(Debug, Clone, Serialize)]
pub struct Supplied {
    /// The declaration's value.
    pub value: Value,
    /// The configuration layer that supplied it.
    pub layer: Layer,
}
/// Only the keys a verb actually uses are included in its answer.
pub type Used = BTreeMap<String, Supplied>;

/// A validated declaration. Missing keys remain missing.
#[derive(Debug)]
pub struct Policy(Used);
fn invalid(key: &str) -> String {
    format!("malformed landing.{key}")
}
fn strings(value: &Value) -> bool {
    value.as_array().is_some_and(|items| {
        !items.is_empty()
            && items
                .iter()
                .all(|v| v.as_str().is_some_and(|s| !s.is_empty()))
    })
}
impl Policy {
    /// Validate the entire landing block before selecting any keys.
    pub fn parse(block: Option<&Value>) -> Result<Self, String> {
        let mut values = Used::new();
        let Some(block) = block else {
            return Ok(Self(values));
        };
        let object = block
            .as_object()
            .ok_or_else(|| "malformed landing".to_owned())?;
        for (key, value) in object {
            if key == "stack" {
                let stack = value.as_object().ok_or_else(|| invalid(key))?;
                for (key, value) in stack {
                    let key = format!("stack.{key}");
                    let valid = match key.as_str() {
                        "stack.depth" | "stack.max_stacked" => {
                            value.as_u64().is_some_and(|n| u32::try_from(n).is_ok())
                        }
                        "stack.depth_by_spec" => value.as_object().is_some_and(|map| {
                            map.iter().all(|(id, v)| {
                                !id.is_empty()
                                    && v.as_u64().is_some_and(|n| u32::try_from(n).is_ok())
                            })
                        }),
                        _ => false,
                    };
                    if !valid {
                        return Err(invalid(&key));
                    }
                    values.insert(
                        key,
                        Supplied {
                            value: value.clone(),
                            layer: Layer::ProjectOverride,
                        },
                    );
                }
                continue;
            }
            let valid = match key.as_str() {
                "default_branch" => value.as_str().is_some_and(|s| !s.trim().is_empty()),
                "pre_push" => value
                    .as_array()
                    .is_some_and(|items| items.iter().all(strings)),
                "generated" => value.as_array().is_some_and(|items| {
                    items.iter().all(|entry| {
                        entry.as_object().is_some_and(|obj| {
                            obj.len() == 2
                                && strings(&entry["paths"])
                                && strings(&entry["regenerate"])
                        })
                    })
                }),
                _ => false,
            };
            if !valid {
                return Err(invalid(key));
            }
            values.insert(
                key.clone(),
                Supplied {
                    value: value.clone(),
                    layer: Layer::ProjectOverride,
                },
            );
        }
        Ok(Self(values))
    }

    /// Read the committed declaration at HEAD, not an uncommitted policy edit.
    pub fn read(directory: &Path) -> Result<Self, String> {
        let output = Command::new("git")
            .args(["show", "HEAD:.statecraft/environment.json"])
            .current_dir(directory)
            .output()
            .map_err(|_| "cannot read committed landing declaration".to_owned())?;
        if !output.status.success() {
            return Err(
                "cannot read committed landing declaration: landing.default_branch unavailable"
                    .into(),
            );
        }
        let declaration = Manifest::parse(&output.stdout)
            .map_err(|_| "malformed landing declaration".to_owned())?;
        Self::from_project(&declaration.project)
    }

    /// Validate the block, then take values and provenance from config show's
    /// project-layer resolver. An unmet requirement remains a refusal.
    pub fn from_project(project: &Project) -> Result<Self, String> {
        Self::parse(project.landing.as_ref())?;
        let mut values = Used::new();
        for (key, answer) in authority::landing_configuration(project) {
            let authority::Answer::Resolved {
                value, supplied_by, ..
            } = answer
            else {
                return Err(format!("{key}: {}", answer.describe()));
            };
            let key = key.strip_prefix("landing.").expect("landing key");
            let value = if key == "default_branch" {
                Value::String(value)
            } else {
                serde_json::from_str(&value).map_err(|_| invalid(key))?
            };
            values.insert(
                key.to_owned(),
                Supplied {
                    value,
                    layer: supplied_by,
                },
            );
        }
        Ok(Self(values))
    }

    /// Resolve exactly the requested keys, refusing the first unsupplied key.
    pub fn require(&self, keys: &[&str]) -> Result<Used, String> {
        keys.iter()
            .map(|key| {
                self.0
                    .get(*key)
                    .cloned()
                    .map(|v| ((*key).to_owned(), v))
                    .ok_or_else(|| format!("unsupplied landing.{key}"))
            })
            .collect()
    }
}
