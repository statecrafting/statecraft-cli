//! One attempt's witness sidecar, start to closing outcome (spec 013).
//!
//! `run --witness` requests capture and `run --require-witness` also requires
//! it (section 7, 2026-10-17). The producer is located before the child's
//! admission, so a witnessed admission can reserve the endpoint; the sidecar
//! is admitted, its effect intent written and the process started after every
//! preflight, and the child is given the endpoint only from a validated ready
//! message. The sidecar is closed after the child and its objects, and the
//! attempt records the one posture section 3.7 derives.
//!
//! Spec 014, through its additive edge: the attempt's wire-evidence policy is
//! recorded before the sidecar starts, and once the effect's closing outcome
//! is durable the producer's evidence reference and manifest are judged and
//! one admission record is appended. The manifest is read only from the
//! attempt's sidecar area and only within the section 3.1 bound.

use statecraft_acceptance::wire::{Candidate, Unjudged};
use statecraft_adapter::boundary::Record;
use statecraft_adapter::supervisor::{Control, Watch};
use statecraft_adapter::witness::ledger::VerifiedProducer;
use statecraft_adapter::witness::posture::{Observed, Requirement, derive};
use statecraft_adapter::witness::protocol::{
    AttemptBinding, Closing, Integrity, Limits, Reader, Start, Termination,
};
use statecraft_adapter::witness::sidecar::{Ended, Plan, Running, start_confined};
use statecraft_adapter::witness::{WitnessPosture, effect};
use statecraft_envelope::wire::manifest::Completeness;
use statecraft_envelope::wire::policy::{
    DimensionName, Requirement as WireRequirement, WirePolicy,
};
use statecraft_envelope::wire::record::{Custody as WireCustody, Handoff};
use statecraft_envelope::wire::screen::{DEFAULT_DETECTOR, DefaultDetector};
use statecraft_envelope::wire::{EVIDENCE_TYPE, Stated};
use statecraft_home::witness::{SidecarAdmission, SidecarRoots, admit_sidecar};
use statecraft_run::record::Chain;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The refusal guard of an attempt whose sidecar failed before the child.
pub const GUARD: &str = "witness";

/// How long the sidecar has to report ready, and to finish once stdin closes.
const READY_TIMEOUT: Duration = Duration::from_secs(20);
const GRACE: Duration = Duration::from_secs(10);

/// The Linux residuals a sidecar record must name; without them no posture
/// is qualified (section 4).
const SIDECAR_RESIDUALS: [&str; 3] = [
    "sidecar-bind-address-unenforced",
    "sidecar-connect-address-unenforced",
    "sidecar-udp-unmediated",
];

/// The system's root bundle, which the producer's TLS library reads instead
/// of asking the platform's trust services: the sidecar's confinement refuses
/// their Mach lookups on macOS, as it does the child's.
#[cfg(target_os = "macos")]
const SYSTEM_ROOTS: &str = "/etc/ssl/cert.pem";

/// One attempt's witnessing, from the request to the closing outcome.
pub struct Witness {
    requirement: Requirement,
    producer: Option<Result<VerifiedProducer, String>>,
    binding: Option<AttemptBinding>,
    sidecar: Option<Record>,
    live: Option<(Running, SidecarAdmission)>,
    observed: Observed,
    last: Option<Ended>,
    failure: Option<String>,
    closed: bool,
    area: Option<PathBuf>,
    wire: Option<Result<String, String>>,
}

impl Witness {
    /// Locate the producer if witnessing was requested. Nothing is started.
    #[must_use]
    pub fn resolve(home: &Path, requirement: Requirement) -> Self {
        let producer = requirement
            .requested
            .then(|| statecraft_home::witness::locate(home).map_err(|e| e.to_string()));
        Self {
            requirement,
            producer,
            binding: None,
            sidecar: None,
            live: None,
            observed: Observed::NotStarted,
            last: None,
            failure: None,
            closed: false,
            area: None,
            wire: None,
        }
    }

    /// The attempt's wire-evidence policy (spec 014 section 3.5): the shipped
    /// detector always, and under `--require-witness` a requirement for a
    /// complete artifact whose integrity and subject binding pass.
    #[must_use]
    pub fn wire_policy(&self) -> WirePolicy {
        WirePolicy {
            secret_detector: Some(DEFAULT_DETECTOR.to_string()),
            requirement: self.requirement.required.then(|| WireRequirement {
                artifact_type: EVIDENCE_TYPE.to_string(),
                minimum_completeness: Completeness::Complete,
                required_dimensions: vec![DimensionName::Integrity, DimensionName::SubjectBinding],
                permitted_absence: Vec::new(),
            }),
        }
    }

    /// Whether the child's admission reserves an endpoint: requested, and a
    /// producer was found to bind it.
    #[must_use]
    pub fn reserves(&self) -> bool {
        matches!(self.producer, Some(Ok(_)))
    }

    /// Admit and start the sidecar, write its effect intent first, and
    /// return the child's environment additions. `Ok(None)` means nothing
    /// was to start (not requested, or the producer unavailable) and the
    /// child runs unwitnessed; `Err` means witnessing failed after the
    /// endpoint was reserved and the child must not be spawned.
    ///
    /// # Errors
    ///
    /// Why the child must not be spawned.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        chain: &mut Chain,
        timestamp: &str,
        roots: &Roots<'_>,
        run_id: &str,
        attempt: u32,
        child: &mut statecraft_home::confinement::Admission,
        probe_executable: &Path,
    ) -> Result<Option<BTreeMap<String, String>>, String> {
        let Some(Ok(producer)) = &self.producer else {
            return Ok(None);
        };
        let producer = producer.clone();
        let Some(endpoint) = child.witness_endpoint() else {
            return self.refuse(Observed::Unavailable, "no endpoint was reserved");
        };
        let Some(binding) = effect::mint(run_id, attempt) else {
            return self.refuse(Observed::Unavailable, "no effect identity");
        };
        let area = roots.exchange.join(statecraft_home::witness::AREA);
        // The sidecar's own self-test binds the endpoint, so the child's
        // reservation ends here; the child is retested once the sidecar is ready.
        child.release_witness_endpoint();
        let admission = match admit_sidecar(
            &SidecarRoots {
                home: roots.home,
                records: roots.records,
                target: roots.target,
                workspace: roots.workspace,
                area: &area,
                producer: &producer,
            },
            probe_executable,
            endpoint,
            statecraft_adapter_claude_code::witness::OUTBOUND_PORTS,
        ) {
            Ok(admission) => admission,
            // Section 3.6.2: a confinement failure refuses witness startup.
            Err(why) => {
                return self.refuse(
                    Observed::Unavailable,
                    &format!("sidecar confinement: {why}"),
                );
            }
        };
        self.sidecar = Some(admission.boundary.record().clone());
        self.area = Some(admission.output.clone());
        // Spec 014 section 3.5: the policy is recorded once, before the
        // sidecar starts, so admission judges under what was asked for.
        let target = statecraft_run::wire::Target {
            run_id: run_id.to_string(),
            attempt,
        };
        if let Err(e) =
            statecraft_run::wire::record_policy(chain, &target, &self.wire_policy(), timestamp)
        {
            return self.refuse(Observed::Unavailable, &format!("wire-evidence policy: {e}"));
        }
        // Section 3.2: the intent is durable before the sidecar starts.
        if let Err(e) = effect::open(chain, &binding, &producer, timestamp) {
            return self.refuse(Observed::Unavailable, &format!("effect intent: {e}"));
        }
        self.binding = Some(binding.clone());
        let plan = Plan {
            start: Start {
                binding,
                capture_authorities: statecraft_adapter_claude_code::witness::capture_authorities(),
                authentication_tunnels: Vec::new(),
                requested_retention: "metadata-only".into(),
                limits: Limits::default(),
                output_directory: admission.output.clone(),
                certificate_path: admission.output.join("ca.pem"),
                host_policy_id: format!("statecraft/{}", admission.boundary.record().digest),
                listen_endpoint: endpoint,
            },
            producer,
            wrapper: Vec::new(),
            environment: sidecar_environment(&admission),
            ready_timeout: READY_TIMEOUT,
            grace: GRACE,
        };
        let running = match start_confined(&plan, &admission.boundary) {
            Ok(running) => running,
            Err(Ok(failed)) => return self.refuse(Observed::Unavailable, &failed.to_string()),
            Err(Err(ended)) => {
                let why = ended
                    .refusal
                    .clone()
                    .unwrap_or_else(|| "sidecar failed".into());
                self.ended(&ended);
                return self.refuse(Observed::Failed, &why);
            }
        };
        let ready = running.ready().clone();
        self.live = Some((running, admission));
        // Section 3.3: the child's boundary is proved again against the
        // sidecar now listening on its endpoint.
        if let Err(why) = child.retest(probe_executable) {
            self.abort("child boundary retest");
            return Err(format!("child boundary retest: {why}"));
        }
        Ok(Some(
            statecraft_adapter_claude_code::witness::child_variables(
                ready.endpoint,
                &ready.certificate_path,
            ),
        ))
    }

    /// A watch that forwards to `inner` and stops the child when the sidecar
    /// reports a custody violation (section 3.5).
    pub fn custody<'a, E>(&'a mut self, inner: &'a mut dyn Watch<E>) -> Custody<'a, E> {
        Custody {
            inner,
            witness: self,
        }
    }

    /// End custody after the child and its objects: close the sidecar's
    /// stdin and read its terminal message. A no-op when none is live.
    pub fn close(&mut self) {
        if let Some((running, _admission)) = self.live.take() {
            let ended = running.close();
            self.ended(&ended);
        }
    }

    /// Stop a live sidecar now.
    pub fn abort(&mut self, why: &str) {
        if let Some((running, _admission)) = self.live.take() {
            let ended = running.abort(why);
            self.failure.get_or_insert_with(|| why.to_string());
            self.ended(&ended);
        }
    }

    /// The one posture this attempt records.
    #[must_use]
    pub fn posture(&self) -> WitnessPosture {
        derive(self.requirement, self.observed)
    }

    /// Write the effect's one closing outcome, if an intent was written.
    pub fn write_outcome(&mut self, chain: &mut Chain, timestamp: &str) -> Option<String> {
        self.close();
        let binding = self.binding.clone()?;
        if self.closed {
            return None;
        }
        let (termination, integrity, reader) = match &self.last {
            Some(last) => (last.termination, last.integrity, last.reader.clone()),
            None => (
                Termination::Unknown,
                Integrity::Missing,
                Reader::new(binding.clone()),
            ),
        };
        let producer = match &self.producer {
            Some(Ok(p)) => (format!("{}@{}", p.name, p.version), p.interface.clone()),
            _ => (String::new(), String::new()),
        };
        let closing = Closing::assemble(
            &reader,
            termination,
            integrity,
            &producer.0,
            &producer.1,
            self.posture(),
        );
        self.closed = true;
        let closed = effect::close(chain, &binding, &closing, timestamp);
        if closed.is_ok() {
            self.admit(chain, &binding, timestamp);
        }
        closed.err().map(|e| e.to_string())
    }

    /// Spec 014 section 3.6 over the producer's reference and manifest, once
    /// the closing outcome is durable. Nothing to judge when the producer
    /// named no manifest; an unreadable one is `unavailable` and records no
    /// admission.
    fn admit(&mut self, chain: &mut Chain, binding: &AttemptBinding, timestamp: &str) {
        let Some(last) = &self.last else {
            return;
        };
        let Some(terminal) = last.reader.terminal() else {
            return;
        };
        let (Some(reference), Some(path)) = (&terminal.evidence_reference, &terminal.manifest_path)
        else {
            return;
        };
        // Only a manifest inside the sidecar area is read; a path elsewhere
        // is never followed.
        if !matches!(last.integrity, Integrity::Intact | Integrity::Mismatch) {
            self.wire = Some(Err(format!("manifest {:?}", last.integrity)));
            return;
        }
        let manifest = match self.area.as_deref().map(|area| read_bounded(area, path)) {
            Some(Ok(bytes)) => bytes,
            Some(Err(why)) => {
                self.wire = Some(Err(why));
                return;
            }
            None => {
                self.wire = Some(Err("no sidecar area".into()));
                return;
            }
        };
        let reference = serde_json::to_vec(reference).unwrap_or_default();
        let writer = match &self.producer {
            Some(Ok(p)) => Stated::known(format!("{}@{}", p.name, p.version)),
            _ => Stated::Unknown,
        };
        let candidate = Candidate {
            reference: &reference,
            manifest: &manifest,
            custody: WireCustody {
                obtained_from: Stated::known(path.clone()),
                writer,
                reader: Stated::known(format!(
                    "statecraft-cli@{} pid {}",
                    env!("CARGO_PKG_VERSION"),
                    std::process::id()
                )),
                handoff: if last.termination == Termination::Finished {
                    Handoff::Complete
                } else {
                    Handoff::Incomplete
                },
            },
        };
        let target = statecraft_run::wire::Target {
            run_id: binding.run_id.clone(),
            attempt: binding.attempt,
        };
        let detector = DefaultDetector::default();
        self.wire = Some(
            match statecraft_acceptance::wire::admit(
                chain,
                &target,
                &candidate,
                Some(&detector),
                timestamp,
            ) {
                Ok(record) => Ok(format!(
                    "{} {}",
                    record.outcome.word(),
                    record.reason.word()
                )),
                Err(Unjudged(why)) => Err(why),
            },
        );
    }

    /// `detail`, [`Witness::mark`]ed.
    #[must_use]
    pub fn marked(&self, mut detail: serde_json::Value) -> serde_json::Value {
        self.mark(&mut detail);
        detail
    }

    /// Add `witnessPosture`, `witnessRequired` and the witness record to an
    /// attempt's outcome detail.
    pub fn mark(&self, detail: &mut serde_json::Value) {
        let Some(map) = detail.as_object_mut() else {
            return;
        };
        map.insert("witnessPosture".into(), self.posture().word().into());
        map.insert("witnessRequired".into(), self.requirement.required.into());
        if !self.requirement.requested {
            return;
        }
        map.insert(
            "witness".into(),
            serde_json::json!({
                "effectId": self.binding.as_ref().map(|b| b.effect_id.clone()),
                "producer": match &self.producer {
                    Some(Ok(p)) => serde_json::to_value(p).unwrap_or_default(),
                    Some(Err(why)) => serde_json::json!({ "unavailable": why }),
                    None => serde_json::Value::Null,
                },
                "sidecarConfinement": self.sidecar,
                "failure": self.failure,
                "wireEvidence": match &self.wire {
                    Some(Ok(judged)) => serde_json::json!({ "judged": judged }),
                    Some(Err(why)) => serde_json::json!({ "unavailable": why }),
                    None => serde_json::json!("not-recorded"),
                },
            }),
        );
    }

    fn ended(&mut self, ended: &Ended) {
        let qualified = self.sidecar.as_ref().is_some_and(qualified);
        self.observed = ended.observed(qualified);
        self.last = Some(ended.clone());
    }

    fn refuse(
        &mut self,
        observed: Observed,
        why: &str,
    ) -> Result<Option<BTreeMap<String, String>>, String> {
        if matches!(self.observed, Observed::NotStarted) {
            self.observed = observed;
        }
        self.failure = Some(why.to_string());
        Err(why.to_string())
    }
}

/// The manifest at `path`, read only when it is a regular file inside `area`
/// and only up to one byte past spec 014's bound, so an oversized file is
/// refused by its reference's length or its digest, never read whole.
fn read_bounded(area: &Path, path: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(path);
    if !path.starts_with(area)
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("manifest outside the sidecar area".into());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|e| format!("manifest: {e}"))?;
    if !metadata.file_type().is_file() {
        return Err("manifest is not a regular file".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|f| {
            f.take(statecraft_envelope::wire::bounded::MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|e| format!("manifest: {e}"))?;
    Ok(bytes)
}

/// The paths a sidecar is admitted against.
pub struct Roots<'a> {
    /// Protected product home.
    pub home: &'a Path,
    /// Protected launch records.
    pub records: &'a Path,
    /// The operator checkout.
    pub target: &'a Path,
    /// The child's workspace.
    pub workspace: &'a Path,
    /// The attempt's exchange directory, which holds the sidecar area.
    pub exchange: &'a Path,
}

/// Section 4: on Linux a record without the address and UDP residuals is
/// unqualified; macOS enforces the addresses.
fn qualified(record: &Record) -> bool {
    !cfg!(target_os = "linux")
        || SIDECAR_RESIDUALS
            .iter()
            .all(|r| record.open_items.iter().any(|i| i.starts_with(r)))
}

/// [`Witness::custody`]'s watch.
pub struct Custody<'a, E> {
    inner: &'a mut dyn Watch<E>,
    witness: &'a mut Witness,
}

impl<E> Watch<E> for Custody<'_, E> {
    fn spawned(&mut self, pid: u32) -> Result<(), String> {
        self.inner.spawned(pid)
    }

    fn event(&mut self, event: &E) -> Control {
        if let Some((running, _)) = self.witness.live.as_mut()
            && let Some(finding) = running.poll()
        {
            let why = format!("custody violation: {finding:?}");
            self.witness.abort(&why);
            return Control::Stop(why);
        }
        self.inner.event(event)
    }
}

/// The sidecar's whole environment.
fn sidecar_environment(admission: &SidecarAdmission) -> BTreeMap<String, String> {
    #[allow(unused_mut)]
    let mut environment = BTreeMap::from([
        ("PATH".into(), "/usr/bin:/bin".into()),
        ("TMPDIR".into(), admission.temporary.display().to_string()),
    ]);
    #[cfg(target_os = "macos")]
    environment.insert("SSL_CERT_FILE".into(), SYSTEM_ROOTS.into());
    environment
}
