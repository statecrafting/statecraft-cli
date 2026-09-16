//! What every attempt reports about how it actually ran.
//!
//! Spec 004 section 3.7. The attempt record and the outcome both carry the
//! adapter name and version, its qualification state, the capabilities
//! requested, applied and degraded, and whether the constructed environment was
//! applied, degraded or refused, **with each residual named**.
//!
//! An operator never has to ask what posture a run actually had.

use crate::capability::Capability;
use crate::environment::{ChildEnvironment, EnvironmentState, RESIDUALS};
use crate::manifest::{Manifest, Qualification};
use serde::{Deserialize, Serialize};

/// The posture of one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posture {
    /// The adapter's name.
    pub adapter: String,
    /// Its binary version.
    pub adapter_version: String,
    /// Qualified, or running and labelled unqualified.
    pub qualification: Qualification,
    /// What the run asked to require.
    pub required: Vec<Capability>,
    /// What it asked to prefer.
    pub preferred: Vec<Capability>,
    /// What the provider's init event said it actually applied.
    ///
    /// Recorded **as observed**, never as declared.
    pub applied: Vec<Capability>,
    /// What the negotiation degraded.
    pub degraded: Vec<Capability>,
    /// Applied, degraded or refused.
    pub environment: EnvironmentState,
    /// Every residual of the constructed environment, named.
    pub residuals: Vec<String>,
    /// True when the attempt's refusal account cannot be verified.
    pub unverifiable_refusal_account: bool,
    /// Set when a child left a process behind after the kill.
    ///
    /// Section 3.8: reported as a residual on the attempt, **never reported as
    /// a clean termination**.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surviving_processes: Option<String>,
}

impl Posture {
    /// Build a posture from the parts an attempt already has.
    pub fn new(
        manifest: &Manifest,
        qualification: Qualification,
        requested: &crate::capability::Requested,
        negotiation: &crate::capability::Negotiation,
        applied: &[Capability],
        environment: &ChildEnvironment,
    ) -> Self {
        Self {
            adapter: manifest.adapter.clone(),
            adapter_version: manifest.version.clone(),
            qualification,
            required: requested.required.clone(),
            preferred: requested.preferred.clone(),
            applied: applied.to_vec(),
            degraded: negotiation.degraded.clone(),
            environment: environment.state.clone(),
            residuals: RESIDUALS.iter().map(|s| (*s).to_string()).collect(),
            unverifiable_refusal_account: negotiation.unverifiable_refusal_account(),
            surviving_processes: None,
        }
    }

    /// Note that a process outlived the kill.
    #[must_use]
    pub fn with_surviving_processes(mut self, detail: &str) -> Self {
        self.surviving_processes = Some(detail.to_string());
        self
    }

    /// Whether this attempt may be described as cleanly terminated.
    ///
    /// A survivor makes the answer no, whatever else went right.
    pub fn clean_termination(&self) -> bool {
        self.surviving_processes.is_none()
    }

    /// A rendering an operator can read without asking a follow-up question.
    pub fn render(&self) -> String {
        let mut out = format!(
            "adapter {} {} ({})\n",
            self.adapter,
            self.adapter_version,
            self.qualification.word()
        );
        out.push_str(&format!(
            "capabilities required [{}] preferred [{}] applied [{}] degraded [{}]\n",
            tokens(&self.required),
            tokens(&self.preferred),
            tokens(&self.applied),
            tokens(&self.degraded)
        ));
        out.push_str(&match &self.environment {
            EnvironmentState::Applied => "environment applied\n".to_string(),
            EnvironmentState::Degraded { reasons } => {
                format!("environment degraded: {}\n", reasons.join("; "))
            }
            EnvironmentState::Refused { reasons } => {
                format!("environment refused: {}\n", reasons.join("; "))
            }
        });
        if self.unverifiable_refusal_account {
            out.push_str("refusal account: UNVERIFIABLE (structured-refusals degraded)\n");
        }
        if let Some(s) = &self.surviving_processes {
            out.push_str(&format!("residual: a process outlived the kill: {s}\n"));
        }
        for r in &self.residuals {
            out.push_str(&format!("residual: {r}\n"));
        }
        out
    }
}

fn tokens(cs: &[Capability]) -> String {
    cs.iter().map(|c| c.token()).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::{Requested, negotiate};
    use crate::environment::{Blueprint, CheckSuiteCommands, construct};

    fn parts() -> (Manifest, ChildEnvironment) {
        (
            Manifest {
                adapter: "fixture".into(),
                version: "1.0.0".into(),
                supports: vec![Capability::TurnLimit],
                requires_commands: vec![],
            },
            construct(&Blueprint::empty(), &CheckSuiteCommands::default()),
        )
    }

    #[test]
    fn a_posture_names_every_residual_even_when_everything_applied() {
        let (m, env) = parts();
        let requested = Requested::none().requiring(Capability::TurnLimit);
        let n = negotiate(&requested, &m.supports);
        let p = Posture::new(
            &m,
            Qualification::Qualified,
            &requested,
            &n,
            &[Capability::TurnLimit],
            &env,
        );
        assert_eq!(p.residuals.len(), RESIDUALS.len());
        assert!(p.render().contains("residual:"));
    }

    #[test]
    fn an_unqualified_adapter_is_labelled_in_the_posture() {
        let (m, env) = parts();
        let requested = Requested::none();
        let n = negotiate(&requested, &m.supports);
        let p = Posture::new(&m, Qualification::Unqualified, &requested, &n, &[], &env);
        assert!(p.render().contains("unqualified"));
    }

    #[test]
    fn a_degraded_structured_refusals_token_labels_the_account_unverifiable() {
        let (m, env) = parts();
        let requested = Requested::none().preferring(Capability::StructuredRefusals);
        let n = negotiate(&requested, &m.supports);
        let p = Posture::new(&m, Qualification::Qualified, &requested, &n, &[], &env);
        assert!(p.unverifiable_refusal_account);
        assert!(p.render().contains("UNVERIFIABLE"));
    }

    #[test]
    fn a_surviving_process_is_a_residual_and_not_a_clean_termination() {
        let (m, env) = parts();
        let requested = Requested::none();
        let n = negotiate(&requested, &m.supports);
        let p = Posture::new(&m, Qualification::Qualified, &requested, &n, &[], &env)
            .with_surviving_processes("pid 4242 in the process group");
        assert!(!p.clean_termination());
        assert!(p.render().contains("outlived the kill"));
    }

    #[test]
    fn the_applied_set_is_what_was_observed_not_what_was_declared() {
        let (m, env) = parts();
        let requested = Requested::none().requiring(Capability::TurnLimit);
        let n = negotiate(&requested, &m.supports);
        // Declared turn-limit; the init event applied nothing.
        let p = Posture::new(&m, Qualification::Qualified, &requested, &n, &[], &env);
        assert!(p.applied.is_empty());
        assert_eq!(p.required, [Capability::TurnLimit]);
    }
}
