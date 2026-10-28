//! Spec 063 increment 1: policy and read-only waits through the operator's gh.
mod action;
pub mod arm;
pub mod push;
pub mod restack;
pub use action::{Attempt, UsedPolicy};
mod observation;
pub mod policy;
pub use observation::{Gh, WaitHost};
use serde::Serialize;
use statecraft_home::setup::{HostError, repository_slug};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};

/// The events a caller can wait for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    /// Merged or closed unmerged.
    Merged,
    /// Merged, closed, or dropped while open.
    Landed,
    /// No queue entry remains.
    QueueExit,
    /// The current head has a non-pending review outcome.
    Reviewed,
    /// Any specified observation field differs from the initial observation.
    Changed,
}
impl std::str::FromStr for Event {
    type Err = &'static str;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "merged" => Ok(Self::Merged),
            "landed" => Ok(Self::Landed),
            "queue-exit" => Ok(Self::QueueExit),
            "reviewed" => Ok(Self::Reviewed),
            "changed" => Ok(Self::Changed),
            _ => Err("unknown land wait event"),
        }
    }
}
/// Parse an explicit seconds, minutes, or hours duration. Zero permits one read.
pub fn duration(value: &str) -> Result<Duration, &'static str> {
    let (digits, factor) = match value.as_bytes().last() {
        Some(b's') => (&value[..value.len() - 1], 1),
        Some(b'm') => (&value[..value.len() - 1], 60),
        Some(b'h') => (&value[..value.len() - 1], 3600),
        _ => return Err("timeout requires a duration such as 90s, 30m or 2h"),
    };
    digits
        .parse::<u64>()
        .ok()
        .filter(|_| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|n| n.checked_mul(factor))
        .map(Duration::from_secs)
        .ok_or("invalid timeout duration")
}
/// Time is injected, without a product flag that bypasses the polling interval.
pub trait Clock {
    /// Monotonic time since an arbitrary origin.
    fn now(&self) -> Duration;
    /// Sleep for at least this interval.
    fn sleep(&mut self, duration: Duration);
}
/// The production monotonic clock.
pub struct SystemClock(Instant);
impl Default for SystemClock {
    fn default() -> Self {
        Self(Instant::now())
    }
}
impl Clock for SystemClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
/// Arguments independent of CLI rendering.
pub struct Request<'a> {
    /// Pull request numbers, in the caller's order.
    pub pull_requests: &'a [u64],
    /// Named event.
    pub event: Event,
    /// Explicit maximum wait.
    pub timeout: Duration,
    /// Optional repository, otherwise read origin.
    pub repository: Option<&'a str>,
    /// Local project directory.
    pub directory: &'a Path,
}
/// The queue entry belonging to this pull request, regardless of position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Queue {
    /// Host queue state.
    pub state: String,
    /// Position in the entire queue.
    pub position: u64,
}
/// One successful observation of one pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Observation {
    /// open, closed, or merged.
    pub state: String,
    /// Current head SHA.
    pub head: String,
    /// Auto-merge is requested.
    pub armed: bool,
    /// The pull request's queue entry, or null.
    pub queue: Option<Queue>,
    /// Review outcome when the event requires it, otherwise null.
    pub review: Option<String>,
}
impl Observation {
    fn changed(&self, first: &Self) -> bool {
        self.state != first.state
            || self.head != first.head
            || self.armed != first.armed
            || self.queue.as_ref().map(|q| &q.state) != first.queue.as_ref().map(|q| &q.state)
            || self.review != first.review
    }
    fn met(&self, event: Event, first: &Self) -> Option<(&str, i32)> {
        match event {
            Event::Merged if self.state == "merged" => Some(("merged", 0)),
            Event::Merged if self.state == "closed" => Some(("closed-unmerged", 1)),
            Event::Landed if self.state == "merged" || self.state == "closed" => {
                Some((&self.state, 0))
            }
            Event::Landed if !self.armed && self.queue.is_none() => Some(("dropped", 0)),
            Event::QueueExit if self.queue.is_none() => Some((&self.state, 0)),
            Event::Reviewed if self.review.as_deref().is_some_and(|r| r != "pending") => {
                Some((self.review.as_deref().unwrap(), 0))
            }
            Event::Changed if self.changed(first) => Some(("changed", 0)),
            _ => None,
        }
    }
}
/// A requested pull request and its last successful observation.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LastObservation {
    /// The requested pull request number.
    pub pull_request: u64,
    /// Observation fields are absent until a successful read.
    #[serde(flatten)]
    pub observation: Option<Observation>,
}
/// One answer for both renderings, including failures and partial observations.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    /// Requested event.
    pub event: Event,
    /// Event result, timed-out, refused, usage, or failed.
    pub outcome: String,
    /// The first pull request meeting the event, otherwise null.
    pub pull_request: Option<u64>,
    /// Last observations in first-requested order, one entry per pull request.
    pub observations: Vec<LastObservation>,
    /// Each used landing value and its supplying layer.
    pub policy: policy::Used,
    /// Sanitized refusal or failure reason.
    pub reason: Option<String>,
    /// Spec 006 process exit code.
    #[serde(skip)]
    pub exit_code: i32,
}
impl Answer {
    fn end(mut self, outcome: &str, code: i32, reason: Option<String>) -> Self {
        self.outcome = outcome.into();
        self.exit_code = code;
        self.reason = reason;
        self
    }
}
/// Read policy and wait; this operation never mutates the project or host.
pub fn wait(host: &dyn WaitHost, clock: &mut dyn Clock, request: Request<'_>) -> Answer {
    let policy = policy::Policy::read(request.directory);
    wait_with_policy(host, clock, request, policy)
}
/// The same operation with an already read policy, for embedded callers and tests.
pub fn wait_with_policy(
    host: &dyn WaitHost,
    clock: &mut dyn Clock,
    request: Request<'_>,
    policy: Result<policy::Policy, String>,
) -> Answer {
    let mut answer = Answer {
        event: request.event,
        outcome: String::new(),
        pull_request: None,
        observations: request
            .pull_requests
            .iter()
            .fold(Vec::new(), |mut entries, &pr| {
                if !entries.iter().any(|entry| entry.pull_request == pr) {
                    entries.push(LastObservation {
                        pull_request: pr,
                        observation: None,
                    });
                }
                entries
            }),
        policy: BTreeMap::new(),
        reason: None,
        exit_code: 0,
    };
    let repository = request
        .repository
        .map(str::to_owned)
        .or_else(|| repository_slug(request.directory));
    let Some(repo) = repository.filter(|r| {
        let parts: Vec<_> = r.split('/').collect();
        parts.len() == 2
            && parts.iter().all(|s| {
                !s.is_empty()
                    && *s != "."
                    && *s != ".."
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            })
    }) else {
        return answer.end(
            "usage",
            3,
            Some("expected --repo owner/name or a GitHub origin".into()),
        );
    };
    if request.pull_requests.is_empty() || request.pull_requests.contains(&0) {
        return answer.end(
            "usage",
            3,
            Some("expected positive pull request numbers".into()),
        );
    }
    answer.policy = match policy.and_then(|p| p.require(&["default_branch"])) {
        Ok(p) => p,
        Err(e) => return answer.end("refused", 2, Some(e)),
    };
    let start = clock.now();
    let mut first = BTreeMap::new();
    let mut due = BTreeMap::new();
    let mut failures = BTreeMap::new();
    let mut backoff = BTreeMap::new();
    loop {
        for &pr in request.pull_requests {
            let elapsed = clock.now().saturating_sub(start);
            if due
                .get(&pr)
                .is_some_and(|next| elapsed < *next || elapsed >= request.timeout)
            {
                continue;
            }
            match observation::read(host, &repo, pr, &request) {
                Ok(observation) => {
                    failures.remove(&pr);
                    backoff.remove(&pr);
                    let initial = first.entry(pr).or_insert_with(|| observation.clone());
                    let met = observation
                        .met(request.event, initial)
                        .map(|(s, c)| (s.to_owned(), c));
                    answer
                        .observations
                        .iter_mut()
                        .find(|entry| entry.pull_request == pr)
                        .expect("requested pull request")
                        .observation = Some(observation);
                    if let Some((outcome, code)) = met {
                        answer.pull_request = Some(pr);
                        return answer.end(&outcome, code, None);
                    }
                }
                Err(HostError::NotFound) => {
                    return answer.end(
                        "refused",
                        2,
                        Some(format!("pull request {pr} does not exist")),
                    );
                }
                Err(error) => {
                    let reason = error.reason();
                    if reason.starts_with("rate limited:") {
                        let seconds = backoff.entry(pr).or_insert(60_u64);
                        *seconds = seconds.saturating_mul(2).min(3600);
                    }
                    failures.insert(pr, reason);
                }
            }
            // Schedule from completion: slow reads never compress the interval.
            due.insert(
                pr,
                clock
                    .now()
                    .saturating_sub(start)
                    .saturating_add(Duration::from_secs(*backoff.get(&pr).unwrap_or(&60))),
            );
            // No break at the deadline: a pull request not yet read is still
            // read once (the `due` check skips only those already read), so
            // a `0s` timeout observes each named pull request one time.
        }
        let elapsed = clock.now().saturating_sub(start);
        if elapsed >= request.timeout {
            return if let Some((pr, why)) = failures.first_key_value() {
                answer.end("failed", 4, Some(format!("pull request {pr}: {why}")))
            } else {
                answer.end("timed-out", 1, None)
            };
        }
        let next = due
            .values()
            .min()
            .copied()
            .unwrap_or(request.timeout)
            .min(request.timeout);
        clock.sleep(next.saturating_sub(elapsed));
    }
}

#[cfg(test)]
mod tests;
