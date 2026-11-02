//! What an adapter declares it can do beside the six tokens (spec 047).
//!
//! The six tokens of [`crate::capability`] describe what a provider can assert
//! about a session. A caller also decides things the tokens do not cover:
//! whether the provider's stream can be captured by spec 013's sidecar,
//! whether it runs repository hooks in non-interactive mode, whether it can be
//! routed through a reserved loopback endpoint, whether it takes a per-run
//! configuration directory (spec 042), whether it reads a settings document
//! from a supplied path, and which init field reports its version. Each of
//! those is a field of [`AdapterCapabilities`], and a caller reads the field
//! instead of asking which adapter it holds.
//!
//! **Nothing defaults** (section 3.1). A declaration written in Rust is a
//! struct literal, so the compiler refuses one with a field missing; a
//! declaration supplied as data goes through [`register`], which refuses an
//! unset field by name. There is no `Default`, because the first adapter's
//! behaviour is exactly what a second must not inherit by accident.
//!
//! Section 3.3 is the other half, shaped like spec 004 section 3.3: a field a
//! run **requires** and the adapter declares `false` refuses before spawn
//! with the stable reason [`REASON`], naming the field; a field a run only
//! **prefers** degrades, and the degradation is recorded. Adding a field is an
//! amendment, as adding a token is.

use crate::witness::posture::Requirement;
use serde::{Deserialize, Serialize};

/// The stable reason of a refusal for an undeclared capability field.
pub const REASON: &str = "capability-undeclared";

/// One adapter's capability declaration (spec 047 section 3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdapterCapabilities {
    /// Its provider stream can be captured by spec 013's sidecar.
    pub stream_witness: bool,
    /// Its provider runs repository hooks in non-interactive mode and reports
    /// a blocked action as an event.
    pub print_mode_hooks: bool,
    /// Its provider can be routed through a loopback endpoint the supervisor
    /// reserves.
    pub wire_sidecar: bool,
    /// It accepts a per-run configuration directory (spec 042) instead of the
    /// user's global one.
    pub per_run_config_dir: bool,
    /// It reads a settings document from a file path the supervisor supplies.
    pub settings_file_transport: bool,
    /// The init field it reports the provider version in, or none.
    pub version_field: Option<String>,
}

/// One field of [`AdapterCapabilities`], as a caller names what it needs.
/// Serialized as a string enum value, so kebab-case (spec 006 section 5,
/// 2026-09-25): `stream_witness` is written `stream-witness`; [`Field::name`]
/// is the spec's own spelling, used in messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Field {
    /// [`AdapterCapabilities::stream_witness`].
    StreamWitness,
    /// [`AdapterCapabilities::print_mode_hooks`].
    PrintModeHooks,
    /// [`AdapterCapabilities::wire_sidecar`].
    WireSidecar,
    /// [`AdapterCapabilities::per_run_config_dir`].
    PerRunConfigDir,
    /// [`AdapterCapabilities::settings_file_transport`].
    SettingsFileTransport,
    /// [`AdapterCapabilities::version_field`], declared when it names a field.
    VersionField,
}

impl Field {
    /// Every field, in the order section 3.1 lists them.
    pub const ALL: [Field; 6] = [
        Field::StreamWitness,
        Field::PrintModeHooks,
        Field::WireSidecar,
        Field::PerRunConfigDir,
        Field::SettingsFileTransport,
        Field::VersionField,
    ];

    /// The field as spec 047 writes it, and as a refusal names it.
    pub fn name(self) -> &'static str {
        match self {
            Field::StreamWitness => "stream_witness",
            Field::PrintModeHooks => "print_mode_hooks",
            Field::WireSidecar => "wire_sidecar",
            Field::PerRunConfigDir => "per_run_config_dir",
            Field::SettingsFileTransport => "settings_file_transport",
            Field::VersionField => "version_field",
        }
    }

    /// The key a declaration supplied as data spells it with.
    pub fn key(self) -> &'static str {
        match self {
            Field::StreamWitness => "streamWitness",
            Field::PrintModeHooks => "printModeHooks",
            Field::WireSidecar => "wireSidecar",
            Field::PerRunConfigDir => "perRunConfigDir",
            Field::SettingsFileTransport => "settingsFileTransport",
            Field::VersionField => "versionField",
        }
    }
}

impl AdapterCapabilities {
    /// Whether the declaration says the adapter can do what `field` names.
    pub fn declares(&self, field: Field) -> bool {
        match field {
            Field::StreamWitness => self.stream_witness,
            Field::PrintModeHooks => self.print_mode_hooks,
            Field::WireSidecar => self.wire_sidecar,
            Field::PerRunConfigDir => self.per_run_config_dir,
            Field::SettingsFileTransport => self.settings_file_transport,
            Field::VersionField => self.version_field.is_some(),
        }
    }
}

/// Why a declaration supplied as data was not registered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unregistered {
    /// A field was left unset. Nothing defaults (section 3.1).
    #[error(
        "the capability declaration leaves `{field}` unset; an adapter sets every field and \
         nothing defaults (spec 047 section 3.1)"
    )]
    Unset {
        /// The field, as spec 047 writes it.
        field: &'static str,
    },
    /// The declaration is not an object of the declared fields.
    #[error("the capability declaration is not readable: {detail}")]
    Unreadable {
        /// Why.
        detail: String,
    },
}

/// Register a declaration supplied as data.
///
/// Every field must be present, `versionField` included: an explicit `null`
/// declares that no init field reports the version, and an absent key is
/// refused rather than read as that. A key that is not a field is refused,
/// because a misspelt field is an unset one.
///
/// # Errors
///
/// [`Unregistered::Unset`] naming the first field left unset, in the order
/// section 3.1 lists them, or [`Unregistered::Unreadable`].
pub fn register(declared: &serde_json::Value) -> Result<AdapterCapabilities, Unregistered> {
    let Some(object) = declared.as_object() else {
        return Err(Unregistered::Unreadable {
            detail: "not an object".into(),
        });
    };
    if let Some(field) = Field::ALL
        .into_iter()
        .find(|f| !object.contains_key(f.key()))
    {
        return Err(Unregistered::Unset {
            field: field.name(),
        });
    }
    serde_json::from_value(declared.clone()).map_err(|e| Unregistered::Unreadable {
        detail: e.to_string(),
    })
}

/// What a run needs of a declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needs {
    /// Declared `false`: refused before spawn, naming the field.
    #[serde(default)]
    pub required: Vec<Field>,
    /// Declared `false`: runs, degraded, and recorded.
    #[serde(default)]
    pub preferred: Vec<Field>,
}

impl Needs {
    /// Nothing asked for.
    pub fn none() -> Self {
        Self::default()
    }

    /// Require a field.
    #[must_use]
    pub fn requiring(mut self, field: Field) -> Self {
        self.required.push(field);
        self
    }

    /// Prefer a field.
    #[must_use]
    pub fn preferring(mut self, field: Field) -> Self {
        self.preferred.push(field);
        self
    }

    /// What a witness requirement adds (spec 013, `run --witness` and `run
    /// --require-witness`): the stream must be capturable and the provider
    /// routable through the reserved endpoint. Required under a requirement,
    /// preferred under a request alone, and nothing otherwise.
    #[must_use]
    pub fn witnessing(self, witness: Requirement) -> Self {
        let fields = [Field::StreamWitness, Field::WireSidecar];
        if witness.required {
            fields.into_iter().fold(self, Self::requiring)
        } else if witness.requested {
            fields.into_iter().fold(self, Self::preferring)
        } else {
            self
        }
    }

    /// What a supervised provider launch needs (`run`), the one definition
    /// the launch and the seam's own test both read.
    ///
    /// Every launch hands the provider a per-run configuration directory
    /// (spec 042 section 3.1) and the floor through a settings document
    /// (spec 002 section 3.27), so both are required. A managed launch also
    /// registers its startup hook and admission gate in that document (spec
    /// 002 section 3.32 rule 25), so it requires print-mode hooks. The
    /// witness adds [`Needs::witnessing`].
    #[must_use]
    pub fn launch(witness: Requirement, managed: bool) -> Self {
        let needs = Self::none()
            .requiring(Field::PerRunConfigDir)
            .requiring(Field::SettingsFileTransport);
        let needs = if managed {
            needs.requiring(Field::PrintModeHooks)
        } else {
            needs
        };
        needs.witnessing(witness)
    }
}

/// How a run's needs and a declaration line up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fit {
    /// Required fields the declaration does not declare. Non-empty refuses.
    pub missing_required: Vec<Field>,
    /// Preferred fields the declaration does not declare. Runs degraded.
    pub degraded: Vec<Field>,
}

/// A refusal for an undeclared capability field (section 3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Undeclared {
    /// The adapter, for the record. Named, never compared.
    pub adapter: String,
    /// The required fields it does not declare.
    pub fields: Vec<Field>,
}

impl std::fmt::Display for Undeclared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fields: Vec<&str> = self.fields.iter().map(|field| field.name()).collect();
        write!(
            f,
            "{REASON}: adapter {} does not declare {}; refused before spawn (spec 047 section 3.3)",
            self.adapter,
            fields.join(", ")
        )
    }
}

impl std::error::Error for Undeclared {}

impl Fit {
    /// Whether this must refuse before any process is spawned.
    pub fn refuses(&self) -> bool {
        !self.missing_required.is_empty()
    }

    /// The refusal, when one is due.
    pub fn refusal(&self, adapter: &str) -> Option<Undeclared> {
        self.refuses().then(|| Undeclared {
            adapter: adapter.to_string(),
            fields: self.missing_required.clone(),
        })
    }

    /// The witness requirement a launch proceeds under. A witness that was
    /// only requested, against an adapter that does not declare what
    /// capture needs, is not started: the run proceeds unwitnessed and the
    /// degradation is recorded. A required one is never weakened here; it
    /// is refused by [`Fit::refusal`].
    #[must_use]
    pub fn witness_requirement(&self, witness: Requirement) -> Requirement {
        let degraded = [Field::StreamWitness, Field::WireSidecar]
            .iter()
            .any(|f| self.degraded.contains(f));
        if degraded && !witness.required {
            Requirement::DEFAULT
        } else {
            witness
        }
    }
}

/// Line a run's needs up against a declaration.
pub fn fit(declared: &AdapterCapabilities, needs: &Needs) -> Fit {
    let absent = |fields: &[Field]| -> Vec<Field> {
        fields
            .iter()
            .filter(|f| !declared.declares(**f))
            .copied()
            .collect()
    };
    Fit {
        missing_required: absent(&needs.required),
        degraded: absent(&needs.preferred),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_true() -> serde_json::Value {
        serde_json::json!({
            "streamWitness": true,
            "printModeHooks": true,
            "wireSidecar": true,
            "perRunConfigDir": true,
            "settingsFileTransport": true,
            "versionField": "version",
        })
    }

    #[test]
    fn every_field_has_a_distinct_name_and_key() {
        let mut names: Vec<_> = Field::ALL.iter().map(|f| f.name()).collect();
        let mut keys: Vec<_> = Field::ALL.iter().map(|f| f.key()).collect();
        names.sort_unstable();
        names.dedup();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(names.len(), 6);
        assert_eq!(keys.len(), 6);
    }

    #[test]
    fn a_complete_declaration_registers() {
        let declared = register(&all_true()).expect("registers");
        assert!(Field::ALL.iter().all(|f| declared.declares(*f)));
    }

    #[test]
    fn each_unset_field_is_refused_by_name() {
        for field in Field::ALL {
            let mut value = all_true();
            value.as_object_mut().unwrap().remove(field.key());
            assert_eq!(
                register(&value),
                Err(Unregistered::Unset {
                    field: field.name()
                })
            );
        }
    }

    #[test]
    fn an_explicit_null_version_field_is_set_to_none() {
        let mut value = all_true();
        value["versionField"] = serde_json::Value::Null;
        let declared = register(&value).expect("registers");
        assert!(!declared.declares(Field::VersionField));
    }

    #[test]
    fn a_misspelt_field_is_refused() {
        let mut value = all_true();
        value["streamWitnes"] = true.into();
        assert!(matches!(
            register(&value),
            Err(Unregistered::Unreadable { .. })
        ));
    }

    #[test]
    fn a_preferred_witness_degrades_and_a_required_one_does_not() {
        let mut value = all_true();
        value["streamWitness"] = false.into();
        let declared = register(&value).unwrap();
        let requested = Requirement {
            requested: true,
            required: false,
        };
        let f = fit(&declared, &Needs::none().witnessing(requested));
        assert!(!f.refuses());
        assert_eq!(f.degraded, [Field::StreamWitness]);
        assert_eq!(f.witness_requirement(requested), Requirement::DEFAULT);

        let required = Requirement {
            requested: true,
            required: true,
        };
        let f = fit(&declared, &Needs::none().witnessing(required));
        assert_eq!(f.witness_requirement(required), required);
        let refusal = f.refusal("fixture").expect("refuses");
        assert_eq!(refusal.fields, [Field::StreamWitness]);
        assert!(refusal.to_string().starts_with(REASON));
        assert!(refusal.to_string().contains("stream_witness"));
    }
}
