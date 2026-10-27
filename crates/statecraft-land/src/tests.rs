use super::*;
use serde_json::{Value, json};
use statecraft_home::setup::{Host, HostError};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
};

#[derive(Default)]
struct Time {
    now: Rc<Cell<Duration>>,
    sleeps: Vec<Duration>,
}
impl Clock for Time {
    fn now(&self) -> Duration {
        self.now.get()
    }
    fn sleep(&mut self, d: Duration) {
        self.sleeps.push(d);
        self.now.set(self.now.get() + d);
    }
}
struct Fake {
    replies: RefCell<VecDeque<Result<Value, HostError>>>,
    calls: RefCell<Vec<(u64, Duration)>>,
    now: Rc<Cell<Duration>>,
    review: RefCell<VecDeque<&'static str>>,
    pending: Cell<bool>,
}
impl Host for Fake {
    fn get(&self, path: &str) -> Result<Value, HostError> {
        assert!(
            !path.contains("/pulls/"),
            "PR must come from the same snapshot"
        );
        if path.contains("check-runs") {
            let mut outcomes = self.review.borrow_mut();
            let outcome = if outcomes.len() > 1 {
                outcomes.pop_front().unwrap()
            } else {
                outcomes[0]
            };
            self.pending.set(outcome == "pending");
            if self.pending.get() {
                return Ok(json!({"check_runs":[]}));
            }
            return Ok(
                json!({"check_runs":[{"id":1,"name":"statecraft-ai-review","html_url":"url","output":{"text":format!("```json\n{}\n```", json!({"subject":{"pullRequest":7,"head":"abc"},"result":outcome}))}}]}),
            );
        }
        if path.contains("/jobs") {
            return Ok(
                json!({"jobs":[{"id":2,"name":"ai-review / review","status":"in_progress"}]}),
            );
        }
        assert!(path.contains("statecraft-ci.yml/runs"));
        if self.pending.get() {
            return Ok(
                json!({"workflow_runs":[{"id":1,"run_attempt":1,"head_sha":"abc","status":"in_progress","pull_requests":[{"number":7}]}]}),
            );
        }
        Ok(json!({"workflow_runs":[]}))
    }
}
impl WaitHost for Fake {
    fn pull_request(&self, _: &str, number: u64) -> Result<Value, HostError> {
        self.calls.borrow_mut().push((number, self.now.get()));
        let mut replies = self.replies.borrow_mut();
        if replies.len() > 1 {
            replies.pop_front().unwrap()
        } else {
            replies[0].clone()
        }
    }
}
fn pr(state: &str, armed: bool, position: Option<u64>) -> Value {
    json!({"state":state,"headRefOid":"abc","headRepository":{"nameWithOwner":"o/r"},
        "autoMergeRequest":if armed {json!({"enabledAt":"2026-10-27T00:00:00Z"})} else {Value::Null},
        "mergeQueueEntry":position.map(|p|json!({"state":"QUEUED","position":p}))})
}
fn fixture(replies: Vec<Result<Value, HostError>>) -> (Fake, Time) {
    let clock = Time::default();
    let host = Fake {
        replies: RefCell::new(replies.into()),
        calls: RefCell::new(vec![]),
        now: clock.now.clone(),
        review: RefCell::new(VecDeque::from(["no-findings"])),
        pending: Cell::new(false),
    };
    (host, clock)
}
fn run(host: &Fake, clock: &mut Time, event: Event, seconds: u64, prs: &[u64]) -> Answer {
    wait_with_policy(
        host,
        clock,
        Request {
            pull_requests: prs,
            event,
            timeout: Duration::from_secs(seconds),
            repository: Some("o/r"),
            directory: Path::new("."),
        },
        policy::Policy::parse(Some(&json!({"default_branch":"trunk"}))),
    )
}
#[test]
fn all_immediate_events_and_closed_unmerged() {
    for (event, value, outcome, exit) in [
        (Event::Merged, pr("MERGED", false, None), "merged", 0),
        (
            Event::Merged,
            pr("CLOSED", false, None),
            "closed-unmerged",
            1,
        ),
        (Event::Landed, pr("MERGED", false, None), "merged", 0),
        (Event::Landed, pr("CLOSED", false, None), "closed", 0),
        (Event::Landed, pr("OPEN", false, None), "dropped", 0),
        (Event::QueueExit, pr("OPEN", true, None), "open", 0),
        (
            Event::Reviewed,
            pr("OPEN", true, Some(25)),
            "no-findings",
            0,
        ),
    ] {
        let (host, mut clock) = fixture(vec![Ok(value)]);
        let answer = run(&host, &mut clock, event, 120, &[7]);
        assert_eq!((answer.outcome.as_str(), answer.exit_code), (outcome, exit));
        assert_eq!(answer.pull_request, Some(7));
        assert!(clock.sleeps.is_empty());
    }
}
#[test]
fn changed_observes_each_specified_field_but_not_queue_position() {
    let initial = pr("OPEN", true, Some(25));
    let mut changes = vec![];
    for (key, value) in [
        ("state", json!("CLOSED")),
        ("headRefOid", json!("def")),
        ("autoMergeRequest", Value::Null),
        ("mergeQueueEntry", Value::Null),
    ] {
        let mut next = initial.clone();
        next[key] = value;
        changes.push(next);
    }
    let mut next = initial.clone();
    next["mergeQueueEntry"]["state"] = json!("AWAITING_CHECKS");
    changes.push(next);
    for next in changes {
        let (host, mut clock) = fixture(vec![Ok(initial.clone()), Ok(next)]);
        assert_eq!(
            run(&host, &mut clock, Event::Changed, 90, &[7]).outcome,
            "changed"
        );
        assert_eq!(clock.sleeps, [Duration::from_secs(60)]);
    }
    let (host, mut clock) = fixture(vec![Ok(initial.clone())]);
    *host.review.borrow_mut() = VecDeque::from(["findings", "skipped:transient"]);
    assert_eq!(
        run(&host, &mut clock, Event::Changed, 90, &[7]).outcome,
        "changed"
    );
    let (host, mut clock) = fixture(vec![Ok(initial), Ok(pr("OPEN", true, Some(24)))]);
    assert_eq!(
        run(&host, &mut clock, Event::Changed, 90, &[7]).outcome,
        "timed-out"
    );
}
#[test]
fn position_twenty_five_never_means_dropped_and_timeout_keeps_last_observations() {
    let (host, mut clock) = fixture(vec![Ok(pr("OPEN", false, Some(25)))]);
    let answer = run(&host, &mut clock, Event::Landed, 90, &[8, 7, 8]);
    assert_eq!(answer.outcome, "timed-out");
    assert_eq!(answer.exit_code, 1);
    assert_eq!(
        answer
            .observations
            .iter()
            .map(|entry| entry.pull_request)
            .collect::<Vec<_>>(),
        [8, 7]
    );
    let json = serde_json::to_value(&answer).unwrap();
    assert_eq!(json["observations"][0]["pullRequest"], 8);
    assert_eq!(json["observations"][1]["pullRequest"], 7);
    assert_eq!(json["observations"][0]["state"], "open");
    assert_eq!(
        answer.observations[0]
            .observation
            .as_ref()
            .unwrap()
            .queue
            .as_ref()
            .unwrap()
            .position,
        25
    );
    assert_eq!(
        *host.calls.borrow(),
        [
            (8, Duration::ZERO),
            (7, Duration::ZERO),
            (8, Duration::from_secs(60)),
            (7, Duration::from_secs(60))
        ]
    );
}
#[test]
fn zero_timeout_reads_each_named_pull_request_once() {
    let (host, mut clock) = fixture(vec![Ok(pr("OPEN", false, None))]);
    let answer = run(&host, &mut clock, Event::Merged, 0, &[7, 8, 9, 8]);
    assert_eq!(answer.outcome, "timed-out");
    assert_eq!(answer.exit_code, 1);
    assert_eq!(
        *host.calls.borrow(),
        [
            (7, Duration::ZERO),
            (8, Duration::ZERO),
            (9, Duration::ZERO)
        ]
    );
    assert!(answer.observations.iter().all(|o| o.observation.is_some()));
    assert!(clock.sleeps.is_empty());
}
#[test]
fn first_matching_pr_wins() {
    let (host, mut clock) = fixture(vec![
        Ok(pr("OPEN", true, None)),
        Ok(pr("MERGED", false, None)),
    ]);
    let answer = run(&host, &mut clock, Event::Merged, 90, &[7, 8, 9]);
    assert_eq!(answer.pull_request, Some(8));
    assert!(answer.observations[2].observation.is_none());
    assert_eq!(
        serde_json::to_value(&answer).unwrap()["observations"][2],
        json!({"pullRequest":9})
    );
}
#[test]
fn rate_limit_backoff_recovers_or_fails_at_the_deadline() {
    let rate = Err(HostError::Unreachable("rate limited: HTTP 429".into()));
    let (host, mut clock) = fixture(vec![
        rate.clone(),
        rate.clone(),
        Ok(pr("MERGED", false, None)),
    ]);
    assert_eq!(
        run(&host, &mut clock, Event::Merged, 500, &[7]).outcome,
        "merged"
    );
    assert_eq!(
        clock.sleeps,
        [Duration::from_secs(120), Duration::from_secs(240)]
    );
    let (host, mut clock) = fixture(vec![rate]);
    let answer = run(&host, &mut clock, Event::Merged, 90, &[7]);
    assert_eq!(answer.exit_code, 4);
    assert!(answer.reason.unwrap().contains("HTTP 429"));
    assert_eq!(clock.sleeps, [Duration::from_secs(90)]);
}
#[test]
fn missing_pr_refuses_and_recovered_errors_do_not_poison_timeout() {
    let (host, mut clock) = fixture(vec![Err(HostError::NotFound)]);
    assert_eq!(run(&host, &mut clock, Event::Merged, 90, &[7]).exit_code, 2);
    let (host, mut clock) = fixture(vec![
        Err(HostError::Unauthorized("HTTP 401".into())),
        Ok(pr("OPEN", true, None)),
    ]);
    assert_eq!(run(&host, &mut clock, Event::Merged, 90, &[7]).exit_code, 1);
}
#[test]
fn policies_validate_unused_values_and_require_only_requested_keys() {
    let block = json!({"default_branch":"trunk", "stack":{"depth":0,"max_stacked":3,"depth_by_spec":{"041":2}}, "generated":[{"paths":["generated/**"],"regenerate":["make","refresh"]}],"pre_push":[]});
    let policy = policy::Policy::parse(Some(&block)).unwrap();
    let used = policy.require(&["default_branch"]).unwrap();
    assert_eq!(used.len(), 1);
    assert_eq!(used["default_branch"].layer.word(), "project-override");
    assert_eq!(
        policy.require(&["pre_push"]).unwrap()["pre_push"].value,
        json!([])
    );
    assert!(
        policy::Policy::parse(None)
            .unwrap()
            .require(&["default_branch"])
            .unwrap_err()
            .contains("landing.default_branch")
    );
    for (value, key) in [
        (json!({"default_branch":null}), "default_branch"),
        (json!({"stack":{"depth":-1}}), "stack.depth"),
        (
            json!({"stack":{"max_stacked":4294967296_u64}}),
            "stack.max_stacked",
        ),
        (
            json!({"stack":{"depth_by_spec":{"041":"2"}}}),
            "stack.depth_by_spec",
        ),
        (
            json!({"generated":[{"paths":[],"regenerate":["make"]}]}),
            "generated",
        ),
        (json!({"pre_push":[[]]}), "pre_push"),
        (json!({"unknown":true}), "unknown"),
    ] {
        assert!(
            policy::Policy::parse(Some(&value))
                .unwrap_err()
                .contains(key)
        );
    }
}
#[test]
fn duration_units_and_overflow() {
    for (s, seconds) in [("90s", 90), ("30m", 1800), ("2h", 7200), ("0s", 0)] {
        assert_eq!(duration(s).unwrap().as_secs(), seconds);
    }
    for s in ["", "3", "-1s", "+1s", "1.5h", "18446744073709551615h"] {
        assert!(duration(s).is_err());
    }
}

#[test]
fn reviewed_waits_only_for_pending_and_counts_skips_and_findings() {
    for outcome in [
        "no-findings",
        "findings",
        "skipped:transient",
        "skipped:draft",
    ] {
        let (host, mut clock) = fixture(vec![Ok(pr("OPEN", true, None))]);
        *host.review.borrow_mut() = VecDeque::from(["pending", outcome]);
        let answer = run(&host, &mut clock, Event::Reviewed, 90, &[7]);
        assert_eq!((answer.outcome.as_str(), answer.exit_code), (outcome, 0));
        assert_eq!(clock.sleeps, [Duration::from_secs(60)]);
    }
    let (host, mut clock) = fixture(vec![Ok(pr("OPEN", true, None))]);
    *host.review.borrow_mut() = VecDeque::from(["pending"]);
    let answer = run(&host, &mut clock, Event::Reviewed, 90, &[7]);
    assert_eq!(answer.outcome, "timed-out");
    assert_eq!(
        answer.observations[0]
            .observation
            .as_ref()
            .unwrap()
            .review
            .as_deref(),
        Some("pending")
    );
}

#[test]
fn project_policy_uses_config_show_values_and_layers() {
    use statecraft_environment::manifest::Project;
    use statecraft_home::authority::{Answer, landing_configuration};
    let project = Project {
        landing: Some(json!({"default_branch":"trunk", "stack":{"depth":2,
            "depth_by_spec":{"063":3}}, "pre_push":[["cargo","test"]]})),
        ..Project::default()
    };
    let policy = policy::Policy::from_project(&project).unwrap();
    for (key, answer) in landing_configuration(&project) {
        let Answer::Resolved {
            value, supplied_by, ..
        } = answer
        else {
            panic!("unresolved policy")
        };
        let key = key.strip_prefix("landing.").unwrap();
        let used = policy.require(&[key]).unwrap();
        let supplied = &used[key];
        assert_eq!(supplied.layer, supplied_by);
        assert_eq!(
            supplied
                .value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| supplied.value.to_string()),
            value
        );
    }
    let malformed = Project {
        landing: Some(Value::Null),
        ..Project::default()
    };
    assert!(
        policy::Policy::from_project(&malformed)
            .unwrap_err()
            .contains("malformed landing")
    );
}
