//! One attempt's witness sidecar, start to closing outcome (spec 013).
//!
//! `run --witness` requests capture and `run --require-witness` also requires
//! it (section 7, 2026-10-17). The producer is located before the child's
//! admission, so a witnessed admission can reserve the endpoint; the sidecar
//! is admitted, its effect intent written and the process started after every
//! preflight, and the child is given the endpoint only from a validated ready
//! message. The sidecar is closed after the child and its objects, and the
//! attempt records the one posture section 3.7 derives.

use statecraft_adapter::boundary::Record;
use statecraft_adapter::supervisor::{Control, Watch};
use statecraft_adapter::witness::ledger::VerifiedProducer;
use statecraft_adapter::witness::posture::{Observed, Requirement, derive};
use statecraft_adapter::witness::protocol::{
    AttemptBinding, Closing, Integrity, Limits, Reader, Start, Termination,
};
use statecraft_adapter::witness::sidecar::{Ended, Plan, Running, start_confined};
use statecraft_adapter::witness::{WitnessPosture, effect};
use statecraft_home::witness::{SidecarAdmission, SidecarRoots, admit_sidecar};
use statecraft_run::record::Chain;
use std::collections::BTreeMap;
use std::path::Path;
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
        effect::close(chain, &binding, &closing, timestamp)
            .err()
            .map(|e| e.to_string())
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
