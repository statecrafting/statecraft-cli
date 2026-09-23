//! Whether the contract an attempt was bound to still holds.
//!
//! Spec 005 section 3.18, recorded on 2026-09-22. The binding is spec 003
//! section 3.1.3's, written into the attempt's intent; the resolution now is
//! the producer's answer to the same request. [`compare`] is pure, and every
//! identity it compares is one the producer computed: it names members and
//! quotes their identities, and it computes no digest of its own.

use serde::{Deserialize, Serialize};
use statecraft_run::contract::{Binding, Resolution, State};

/// Rule 2's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Word {
    /// The digest now equals the bound one.
    Current,
    /// The digest differs.
    Changed,
    /// An obligation bound as in force is now withdrawn.
    Withdrawn,
    /// A bound member no longer resolves.
    Missing,
    /// The producer refused a stale ledger; nothing was compared.
    Stale,
    /// Nothing bound, or nothing to compare it with.
    NotRecorded,
}

impl Word {
    /// The word.
    pub fn word(self) -> &'static str {
        match self {
            Word::Current => "current",
            Word::Changed => "changed",
            Word::Withdrawn => "withdrawn",
            Word::Missing => "missing",
            Word::Stale => "stale",
            Word::NotRecorded => "not-recorded",
        }
    }

    /// Whether it is no acceptance, reason `contract-moved`.
    pub fn moved(self) -> bool {
        matches!(self, Word::Changed | Word::Withdrawn | Word::Missing)
    }
}

/// One member whose identity differs, with both identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberChange {
    /// `spec:<id>`, `section:<id>#<anchor>` or `obligation:<id>#<obligation>`.
    pub member: String,
    /// Its identity when bound, `None` where it was not a member then.
    pub bound: Option<String>,
    /// Its identity now, `None` where it is not a member now.
    pub now: Option<String>,
}

/// What `accept` found (rule 3: carried in its answer, beside any receipt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// The word.
    pub word: Word,
    /// The digest the attempt was bound to, where it was bound.
    pub bound_digest: Option<String>,
    /// The digest the producer resolves now, where it resolved.
    pub now_digest: Option<String>,
    /// Every member whose identity differs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<MemberChange>,
    /// Every obligation bound in force and withdrawn now.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub withdrawn: Vec<String>,
    /// Why, in one sentence an operator can act on.
    pub reason: String,
}

impl Comparison {
    /// The human lines.
    pub fn describe(&self) -> String {
        let mut out = format!("contract  {}: {}\n", self.word.word(), self.reason);
        if let Some(d) = &self.bound_digest {
            out.push_str(&format!("  bound     {d}\n"));
        }
        if let Some(d) = &self.now_digest {
            out.push_str(&format!("  now       {d}\n"));
        }
        for c in &self.changed {
            out.push_str(&format!(
                "  changed   {}: {} -> {}\n",
                c.member,
                c.bound.as_deref().unwrap_or("(absent)"),
                c.now.as_deref().unwrap_or("(absent)")
            ));
        }
        for w in &self.withdrawn {
            out.push_str(&format!("  withdrawn {w}\n"));
        }
        out
    }
}

/// A member's key and the identity the producer gave it.
fn keyed(member: &serde_json::Value) -> Option<(String, String)> {
    let kind = member.get("kind")?.as_str()?;
    let spec = member.get("spec")?.as_str()?;
    match kind {
        "spec" => Some((
            format!("spec:{spec}"),
            member.get("contentHash")?.as_str()?.to_string(),
        )),
        "section" => Some((
            format!("section:{spec}#{}", member.get("anchor")?.as_str()?),
            member.get("digest")?.as_str()?.to_string(),
        )),
        "obligation" => {
            // Everything but the key, in the producer's own member: its text,
            // kind, anchor, inputs, withdrawal and section digest. serde_json's
            // map is ordered, so the rendering is stable.
            let mut identity = member.as_object()?.clone();
            for k in ["kind", "spec", "id"] {
                identity.remove(k);
            }
            Some((
                format!("obligation:{spec}#{}", member.get("id")?.as_str()?),
                serde_json::Value::Object(identity).to_string(),
            ))
        }
        other => Some((format!("{other}:{spec}"), member.to_string())),
    }
}

fn withdrawn(member: &serde_json::Value) -> bool {
    member
        .get("withdrawn")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

/// Rule 2. `binding` is the intent's, `None` where the intent carries none;
/// `now` is the producer's answer to the same request, `None` where nothing
/// was asked because nothing was bound.
pub fn compare(binding: Option<&Binding>, now: Option<&Resolution>) -> Comparison {
    let not_recorded = |reason: String, bound: Option<String>| Comparison {
        word: Word::NotRecorded,
        bound_digest: bound,
        now_digest: None,
        changed: Vec::new(),
        withdrawn: Vec::new(),
        reason,
    };
    let Some(binding) = binding else {
        return not_recorded(
            "the attempt's intent carries no contract: it was written before spec 003 \
             section 3.1.3"
                .to_string(),
            None,
        );
    };
    let (Some(bound_digest), State::Bound) = (binding.digest.clone(), binding.state) else {
        return not_recorded(
            format!("the attempt's contract is {}", binding.describe()),
            None,
        );
    };
    let Some(now) = now else {
        return not_recorded(
            "nothing was resolved to compare the bound contract with".to_string(),
            Some(bound_digest),
        );
    };
    match now {
        Resolution::Stale { detail } => Comparison {
            word: Word::Stale,
            bound_digest: Some(bound_digest),
            now_digest: None,
            changed: Vec::new(),
            withdrawn: Vec::new(),
            reason: format!(
                "the producer refused a stale ledger, so nothing was compared; refresh the \
                 ledger and accept again: {detail}"
            ),
        },
        Resolution::Unsupported { detail } | Resolution::Unreadable { detail } => not_recorded(
            format!("the producer answering now did not resolve the bound request: {detail}"),
            Some(bound_digest),
        ),
        Resolution::Unresolved { detail } => Comparison {
            word: Word::Missing,
            bound_digest: Some(bound_digest),
            now_digest: None,
            changed: Vec::new(),
            withdrawn: Vec::new(),
            reason: format!("a bound member no longer resolves: {detail}"),
        },
        Resolution::Resolved { digest, members } => {
            if *digest == bound_digest {
                return Comparison {
                    word: Word::Current,
                    bound_digest: Some(bound_digest),
                    now_digest: Some(digest.clone()),
                    changed: Vec::new(),
                    withdrawn: Vec::new(),
                    reason: "the producer resolves the bound request to the bound digest"
                        .to_string(),
                };
            }
            let before: std::collections::BTreeMap<String, (String, bool)> = binding
                .members
                .iter()
                .filter_map(|m| keyed(m).map(|(k, i)| (k, (i, withdrawn(m)))))
                .collect();
            let after: std::collections::BTreeMap<String, (String, bool)> = members
                .iter()
                .filter_map(|m| keyed(m).map(|(k, i)| (k, (i, withdrawn(m)))))
                .collect();
            let mut changed = Vec::new();
            let mut gone = Vec::new();
            for key in before
                .keys()
                .chain(after.keys())
                .collect::<std::collections::BTreeSet<_>>()
            {
                let (b, a) = (before.get(key), after.get(key));
                if b.map(|x| &x.0) != a.map(|x| &x.0) {
                    changed.push(MemberChange {
                        member: key.clone(),
                        bound: b.map(|x| x.0.clone()),
                        now: a.map(|x| x.0.clone()),
                    });
                }
                if matches!((b, a), (Some((_, false)), Some((_, true)))) {
                    gone.push(key.clone());
                }
            }
            let word = if gone.is_empty() {
                Word::Changed
            } else {
                Word::Withdrawn
            };
            let reason = format!(
                "the producer resolves the bound request to another digest: {} member(s) \
                 changed{}",
                changed.len(),
                if gone.is_empty() {
                    String::new()
                } else {
                    format!(", {} obligation(s) withdrawn", gone.len())
                }
            );
            Comparison {
                word,
                bound_digest: Some(bound_digest),
                now_digest: Some(digest.clone()),
                changed,
                withdrawn: gone,
                reason,
            }
        }
    }
}

/// Rule 1: the contract an attempt's intent recorded, compared with the
/// producer's resolution of the same request now. The one operation `accept`
/// calls; nothing else asks the producer.
pub fn check(
    entries: &[statecraft_run::record::Entry],
    run_id: &str,
    attempt: u32,
    target: &std::path::Path,
    source: &dyn statecraft_run::contract::ContractSource,
) -> Comparison {
    let intent = entries.iter().find(|e| {
        e.kind == statecraft_run::record::Kind::Intent
            && e.run_id == run_id
            && e.attempt == attempt
            && e.subject == statecraft_run::session::INTENT_SUBJECT
    });
    let binding = match intent.and_then(|e| Binding::from_detail(&e.detail)) {
        None => return compare(None, None),
        Some(Err(e)) => {
            return Comparison {
                word: Word::NotRecorded,
                bound_digest: None,
                now_digest: None,
                changed: Vec::new(),
                withdrawn: Vec::new(),
                reason: format!("the attempt's contract could not be read: {e}"),
            };
        }
        Some(Ok(b)) => b,
    };
    let now = match (&binding.state, &binding.request) {
        (State::Bound, Some(request)) => Some(source.resolve(target, request)),
        _ => None,
    };
    compare(Some(&binding), now.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use statecraft_run::contract::{Request, request_for};

    fn obligation(id: &str, text: &str, withdrawn: bool) -> serde_json::Value {
        let mut m = json!({
            "kind": "obligation", "spec": "107-x", "id": id, "obligationKind": "requirement",
            "text": text, "anchor": "a", "sectionDigest": "s"
        });
        if withdrawn {
            m["withdrawn"] = json!(true);
        }
        m
    }

    fn spec(hash: &str) -> serde_json::Value {
        json!({"kind": "spec", "spec": "107-x", "contentHash": hash})
    }

    fn bound(members: Vec<serde_json::Value>) -> Binding {
        Binding::of(
            request_for("107-x", None),
            Resolution::Resolved {
                digest: "d1".into(),
                members,
            },
            "0.22.0",
        )
    }

    #[test]
    fn the_same_digest_is_current() {
        let b = bound(vec![spec("h")]);
        let c = compare(
            Some(&b),
            Some(&Resolution::Resolved {
                digest: "d1".into(),
                members: vec![spec("h")],
            }),
        );
        assert_eq!(c.word, Word::Current);
        assert!(!c.word.moved());
    }

    #[test]
    fn a_changed_obligation_is_named_with_both_identities() {
        let b = bound(vec![spec("h"), obligation("R-1", "old", false)]);
        let c = compare(
            Some(&b),
            Some(&Resolution::Resolved {
                digest: "d2".into(),
                members: vec![spec("h2"), obligation("R-1", "new", false)],
            }),
        );
        assert_eq!(c.word, Word::Changed);
        assert!(c.word.moved());
        let names: Vec<&str> = c.changed.iter().map(|m| m.member.as_str()).collect();
        assert_eq!(names, ["obligation:107-x#R-1", "spec:107-x"]);
        let r1 = &c.changed[0];
        assert!(r1.bound.as_deref().unwrap().contains("\"old\""));
        assert!(r1.now.as_deref().unwrap().contains("\"new\""));
        assert!(c.describe().contains("changed   spec:107-x: h -> h2"));
    }

    #[test]
    fn an_obligation_withdrawn_since_binding_is_withdrawn_and_also_changed() {
        let b = bound(vec![obligation("R-1", "t", false)]);
        let c = compare(
            Some(&b),
            Some(&Resolution::Resolved {
                digest: "d2".into(),
                members: vec![obligation("R-1", "t", true)],
            }),
        );
        assert_eq!(c.word, Word::Withdrawn);
        assert_eq!(c.withdrawn, ["obligation:107-x#R-1"]);
        assert_eq!(c.changed.len(), 1);
    }

    #[test]
    fn an_unresolvable_member_is_missing_and_a_stale_ledger_is_stale() {
        let b = bound(vec![spec("h")]);
        let missing = compare(
            Some(&b),
            Some(&Resolution::Unresolved {
                detail: "spec '107-x': not found".into(),
            }),
        );
        assert_eq!(missing.word, Word::Missing);
        assert!(missing.reason.contains("107-x"));
        let stale = compare(
            Some(&b),
            Some(&Resolution::Stale {
                detail: "stale".into(),
            }),
        );
        assert_eq!(stale.word, Word::Stale);
        assert!(!stale.word.moved());
    }

    #[test]
    fn nothing_bound_or_nothing_resolvable_now_is_not_recorded() {
        assert_eq!(compare(None, None).word, Word::NotRecorded);
        let unsupported = Binding::of(
            Request {
                specs: vec!["107-x".into()],
                obligations: vec![],
            },
            Resolution::Unsupported {
                detail: "exited 3".into(),
            },
            "0.20.0",
        );
        let c = compare(Some(&unsupported), None);
        assert_eq!(c.word, Word::NotRecorded);
        assert!(
            c.reason.contains("spec-spine 0.20.0 was asked"),
            "{}",
            c.reason
        );
        let b = bound(vec![spec("h")]);
        let c = compare(
            Some(&b),
            Some(&Resolution::Unsupported {
                detail: "exited 3".into(),
            }),
        );
        assert_eq!(c.word, Word::NotRecorded);
        assert_eq!(c.bound_digest.as_deref(), Some("d1"));
        assert_eq!(
            compare(Some(&Binding::not_a_unit_of_work()), None).word,
            Word::NotRecorded
        );
    }
}
