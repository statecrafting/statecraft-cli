//! Spec 002's declared acceptance for the managed environment: one test per
//! row of section 3.10 that belongs to the global home and initialization.
//!
//! Each test is named after the row it covers, so a row that stops being
//! covered shows up as a deleted test rather than as a quietly weakened
//! assertion.
//!
//! Every test runs against a temporary product home, a temporary native root
//! and a temporary repository. Nothing here reads or writes the operator's own
//! home, and nothing here reaches a network: the coordination authority in use
//! is the shipped one, which reaches nothing and says so.
//!
//! Where a producer answer is needed, it is the real library's answer
//! restricted to the contract set (see `support::conforming_producer`). That is
//! a fixture and is labelled as one. What the real library returns today is
//! asserted separately, in `tests/producer_integration.rs`.

mod support;

use statecraft_environment::manifest::{Enrollment, Manifest, Pins, Project};
use statecraft_home::authority::{Answer as ConfigAnswer, DEFER_TO_TEAM, Layer, StaticRevision};
use statecraft_home::flow::{Outcome, Step};
use statecraft_home::service::{Answer, Operation, Severity};
use statecraft_home::team::{SharedApproval, Stated, Unreachable};
use statecraft_home::{bridge, delivery, derived, harness, home, project, resolved};
use support::{FixedClock, Harness, Sandbox, StatedProbe, conforming_producer, init_report};

const CLOCK: FixedClock = FixedClock(1_760_000_000);

/// A harness wired to the real corpus tool, a conforming producer, and no
/// platform at all.
macro_rules! harness {
    ($sandbox:expr, $producer:expr, $corpus:expr, $probe:expr, $authority:expr, $revisions:expr) => {
        Harness {
            layout: $sandbox.layout(),
            producer: $producer,
            corpus: $corpus,
            probe: $probe,
            authority: $authority,
            revisions: $revisions,
            clock: CLOCK,
            native_root: $sandbox.native_root(),
        }
    };
}

// Row: fresh solo initialization in a temporary home with no platform
// credential and no platform reachability.
#[test]
fn fresh_solo_initialization_with_no_platform_credential_completes() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::corpus_tool();
    let probe = statecraft_environment::probe::CommandProbe {
        git: "git".into(),
        spec_spine: support::spec_spine_program(),
    };
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);

    // No credential, no token, no account: the environment the flow is given
    // carries none, and the only authority in it reaches nothing.
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);
    assert_eq!(
        report.outcome,
        Outcome::Complete,
        "steps: {:#?}",
        report.steps
    );
    assert_eq!(report.steps.len(), Step::all().len());
    for step in &report.steps {
        assert!(step.state.done(), "{:?} did not complete", step);
    }
    assert_eq!(answer.severity(), Severity::Ok);

    // The four paths of the project area, and the governance starter files.
    assert!(sandbox.exists(project::DECLARATION));
    assert!(sandbox.exists(project::INSTRUCTIONS));
    assert!(sandbox.exists(project::DERIVED));
    assert!(sandbox.exists("spec-spine.toml"));
    assert!(sandbox.exists("standards/spec/constitution.md"));
    assert!(sandbox.exists("specs/000-bootstrap/spec.md"));
}

// Row: `init apply` on an unarmed, unregistered project registers and
// qualifies it and does not arm or execute.
#[test]
fn registration_and_qualification_happen_without_arming_or_execution() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::corpus_tool();
    let probe = statecraft_environment::probe::CommandProbe {
        git: "git".into(),
        spec_spine: support::spec_spine_program(),
    };
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    let registry = statecraft_environment::registry::Registry::read(&sandbox.home()).unwrap();
    let registration = registry
        .get(&sandbox.project())
        .expect("the project is registered");
    assert_eq!(
        registration.qualification.verdict,
        statecraft_environment::qualify::Verdict::Qualified,
        "{}",
        registration.qualification.describe()
    );
    assert!(!registration.armed, "initialization does not arm");
    assert!(
        !registration.eligible(),
        "and therefore nothing is eligible"
    );
    // Nothing executed: there is no run state at all.
    assert!(!sandbox.exists(".statecraft/state/runs"));
    assert!(!sandbox.exists(".statecraft/state/resolved"));
}

// Row: a root `AGENTS.md` that exists and carries the user's own content.
#[test]
fn existing_root_instructions_are_preserved_and_the_import_is_first_and_unique() {
    let sandbox = Sandbox::new();
    let mine = "# My project\n\nOur own protocol. Do not lose this.\n";
    sandbox.write(project::ROOT_INSTRUCTIONS, mine);
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);

    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let after = sandbox.read(project::ROOT_INSTRUCTIONS).unwrap();
    assert_eq!(after.lines().next(), Some(bridge::IMPORT_LINE));
    assert_eq!(after.matches(bridge::IMPORT_LINE).count(), 1);
    assert!(after.contains("Our own protocol. Do not lose this."));
    assert!(after.contains("# My project"));

    // Stable on repetition.
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let again = sandbox.read(project::ROOT_INSTRUCTIONS).unwrap();
    assert_eq!(again, after);
    assert_eq!(again.matches(bridge::IMPORT_LINE).count(), 1);

    // Tracked as a modification, not as ownership of the file.
    let manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    let modification = manifest
        .modification(project::ROOT_INSTRUCTIONS)
        .expect("the bridge is recorded");
    assert_eq!(modification.line, bridge::IMPORT_LINE);
    assert!(
        !manifest.records(project::ROOT_INSTRUCTIONS),
        "the file itself is not owned"
    );
}

// Row: a supported adapter whose harness load rule is evaluable.
#[test]
fn a_supported_adapter_reports_delivery_reached_and_names_the_chain() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);

    // The harness that documents a rule: the chain is evaluated and named.
    let claude = report
        .delivery
        .iter()
        .find(|d| d.harness == "claude-code")
        .expect("the claude-code rule is evaluated");
    match &claude.verdict {
        delivery::Delivery::Reached { via } => {
            assert_eq!(via.last().map(String::as_str), Some(project::INSTRUCTIONS));
            assert!(via.len() >= 2, "the chain has an entry and a destination");
        }
        other => panic!("expected reached, got {}", other.describe()),
    }
}

// Row: a harness with no documented load rule this product can evaluate.
#[test]
fn an_unverified_adapter_is_reported_honestly_and_never_as_delivered() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);

    let codex = report
        .delivery
        .iter()
        .find(|d| d.harness == "codex-cli")
        .expect("the codex rule is reported");
    assert_eq!(codex.verdict.word(), "unverified");
    assert!(!codex.verdict.reached());
    // The file IS there and the import IS first, and that is still not the
    // claim: a file existing is not delivery.
    assert!(sandbox.exists(project::INSTRUCTIONS));
    assert_eq!(
        sandbox
            .read(project::ROOT_INSTRUCTIONS)
            .unwrap()
            .lines()
            .next(),
        Some(bridge::IMPORT_LINE)
    );
}

// Row: a supported managed session needs no project-local generic harness copy.
#[test]
fn no_project_local_generic_harness_copy_is_required_or_written() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);

    for generic in [
        ".claude",
        ".claude/skills",
        ".claude/agents",
        ".claude/rules",
        ".claude/hooks",
        ".agents",
        ".codex",
    ] {
        assert!(
            !sandbox.exists(generic),
            "initialization wrote a project-local harness copy at {generic}"
        );
    }
    for written in &report.writes {
        assert!(
            !written.starts_with(".claude/") && !written.starts_with(".agents/"),
            "{written} is a project-local harness copy"
        );
    }
    // And delivery still reaches the managed instructions without one.
    assert!(
        report
            .delivery
            .iter()
            .any(|d| d.harness == "claude-code" && d.verdict.reached())
    );
}

// Row: existing user-global agent settings present before `home apply`.
#[test]
fn existing_user_global_agent_settings_are_byte_identical_afterwards() {
    let sandbox = Sandbox::new();
    let claude = sandbox.native_root().join(".claude");
    std::fs::create_dir_all(claude.join("agents")).unwrap();
    let settings = claude.join("settings.json");
    let original = b"{\n  \"theme\": \"mine\"\n}\n";
    std::fs::write(&settings, original).unwrap();
    let mine = claude.join("agents/my-own-agent.md");
    std::fs::write(&mine, b"my own agent\n").unwrap();
    let credentials = claude.join(".credentials.json");
    std::fs::write(&credentials, b"secret\n").unwrap();

    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::HomeApply);
    assert_eq!(answer.severity(), Severity::Ok, "{}", answer.render());

    assert_eq!(std::fs::read(&settings).unwrap(), original);
    assert_eq!(std::fs::read(&mine).unwrap(), b"my own agent\n");
    assert_eq!(std::fs::read(&credentials).unwrap(), b"secret\n");
    // What it did do is link the namespaced names, and only those.
    let linked = claude.join("agents/statecraft-acceptance-reader.md");
    assert!(
        std::fs::symlink_metadata(&linked)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
        "the namespaced agent is delivered as a link"
    );
}

// Row: an unrelated repository on the same machine.
#[test]
fn an_unrelated_repository_is_unaffected_and_the_delivered_behavior_is_inert_there() {
    let sandbox = Sandbox::new();
    let unrelated = sandbox.unrelated();
    let before: Vec<_> = std::fs::read_dir(&unrelated)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();

    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::HomeApply);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    let after: Vec<_> = std::fs::read_dir(&unrelated)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after, "the unrelated repository changed");
    assert!(!unrelated.join(".statecraft").exists());

    // The gate every delivered behavior states: outside a Statecraft project
    // it is inert, and this is the predicate it gates on.
    assert!(!project::is_statecraft_project(&unrelated));
    assert!(project::is_statecraft_project(&sandbox.project()));
    for file in harness::shipped() {
        assert!(
            file.contents.contains(".statecraft/environment.json"),
            "{} states no gate",
            file.rel_path
        );
    }
}

// Row: a project override and an explicit run choice.
#[test]
fn a_project_override_and_a_run_choice_are_recorded_with_the_layer_that_supplied_each() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    // A personal default, a project override and a run choice, over three keys.
    let mut personal = home::Personal::default();
    personal.defaults.insert("editor".into(), "vi".into());
    personal.defaults.insert("model".into(), "personal".into());
    personal.write(&sandbox.layout()).unwrap();

    let mut manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    manifest
        .project
        .overrides
        .insert("model".into(), "project".into());
    manifest.write(&sandbox.project()).unwrap();

    let choices = statecraft_home::authority::RunChoices::none().choosing("turns", "3");
    let answer = h.execute(Operation::ConfigShow {
        root: sandbox.project(),
        base_revision: "no-such-revision".into(),
        choices,
    });
    let Answer::Configuration(resolution) = &answer else {
        panic!("expected a configuration, got {answer:?}");
    };
    match &resolution.keys["editor"] {
        ConfigAnswer::Resolved {
            value, supplied_by, ..
        } => {
            assert_eq!(value, "vi");
            assert_eq!(*supplied_by, Layer::PersonalDefault);
        }
        other => panic!("{other:?}"),
    }
    match &resolution.keys["model"] {
        ConfigAnswer::Resolved {
            value, supplied_by, ..
        } => {
            assert_eq!(value, "project");
            assert_eq!(*supplied_by, Layer::ProjectOverride);
        }
        other => panic!("{other:?}"),
    }
    match &resolution.keys["turns"] {
        ConfigAnswer::Resolved {
            value, supplied_by, ..
        } => {
            assert_eq!(value, "3");
            assert_eq!(*supplied_by, Layer::RunChoice);
        }
        other => panic!("{other:?}"),
    }
}

// Row: a global upgrade after a run resolved.
#[test]
fn a_global_upgrade_cannot_alter_a_run_already_resolved() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::HomeApply);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    let before = harness::revision_of(&harness::shipped()).id;
    let resolution = statecraft_home::authority::resolve(
        &home::Personal::default(),
        &statecraft_home::authority::Trusted {
            project: Project::default(),
            source: statecraft_home::authority::Source::TrustedBase {
                revision: "base".into(),
            },
            authority_change: None,
        },
        &statecraft_home::authority::TeamAnswer::NotEnrolled,
        &statecraft_home::authority::RunChoices::none(),
    );
    resolved::ResolvedRun::freeze(
        "003-x",
        &sandbox.project(),
        "1970-01-01T00:00:00Z",
        resolved::Identity {
            name: "harness".into(),
            requested: "latest".into(),
            resolved: before.clone(),
        },
        vec![],
        &resolution,
    )
    .write(&sandbox.project())
    .unwrap();

    // The home upgrades: a different harness revision is installed.
    let mut next = harness::shipped();
    next[0].contents.push_str("\na new line in the harness\n");
    let upgraded = harness::install(&sandbox.layout(), &next).unwrap();
    assert_ne!(upgraded.revision.id, before);

    let frozen = resolved::ResolvedRun::read(&sandbox.project(), "003-x")
        .unwrap()
        .unwrap();
    assert_eq!(
        frozen.harness.resolved, before,
        "the upgrade reached into a run that had already resolved"
    );
    assert_eq!(frozen.harness.requested, "latest");
}

// Row: the real spec-spine library and commands against the new derived path.
#[test]
fn the_real_spec_spine_commands_work_at_the_new_derived_path() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::corpus_tool();
    let probe = statecraft_environment::probe::CommandProbe {
        git: "git".into(),
        spec_spine: support::spec_spine_program(),
    };
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);

    let corpus_step = report
        .steps
        .iter()
        .find(|s| s.step == Step::Corpus)
        .expect("the corpus step ran");
    assert!(
        corpus_step.state.done(),
        "the real tool did not compile, index and check: {corpus_step:?}"
    );

    // The declared layout reached the tool, and the shards landed at the new
    // path rather than at the old one.
    let toml = sandbox.read("spec-spine.toml").unwrap();
    assert!(toml.contains("derived_dir"));
    assert!(toml.contains(".statecraft/derived"));
    assert!(sandbox.exists(".statecraft/derived/spec-registry"));
    assert!(sandbox.exists(".statecraft/derived/codebase-index"));
    assert!(
        !sandbox.exists(".derived"),
        "nothing was written at the old location"
    );
}

// Row: a `.gitignore` that would ignore all of `.statecraft/`.
#[test]
fn a_gitignore_that_would_ignore_the_whole_area_is_refused_with_the_line_named() {
    let sandbox = Sandbox::new();
    sandbox.write(".gitignore", "target\n.statecraft/\n");
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);

    assert_eq!(report.outcome, Outcome::Refused, "{:#?}", report.steps);
    let governance = report
        .steps
        .iter()
        .find(|s| s.step == Step::Governance)
        .expect("the governance step reported");
    let rendered = format!("{:?}", governance.state);
    assert!(rendered.contains(".statecraft/"), "{rendered}");
    assert!(rendered.contains("line 2"), "{rendered}");
    assert!(
        rendered.contains("only .statecraft/state/ is excluded"),
        "{rendered}"
    );
    // And the runtime state is what a correct ignore file excludes.
    assert_eq!(
        statecraft_home::ignore::merge(Some("target\n.statecraft/state/\n"), "")
            .map(|m| m.unchanged),
        Ok(true)
    );
}

// Row: a solo project with a local approval for a subject.
#[test]
fn a_solo_project_with_a_local_approval_is_eligible_on_local_authority() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    let before = h.execute(Operation::ApprovalShow {
        root: sandbox.project(),
        subject: "003-x".into(),
    });
    assert_eq!(before.severity(), Severity::Finding);

    let granted = h.execute(Operation::ApprovalGrant {
        root: sandbox.project(),
        subject: "003-x".into(),
        operator: "bart".into(),
        reason: "reviewed the diff".into(),
    });
    let Answer::Approval(outcome) = &granted else {
        panic!("expected an approval, got {granted:?}");
    };
    assert!(outcome.recorded);
    assert!(outcome.eligibility.eligible());
    assert!(
        outcome
            .eligibility
            .describe()
            .contains("local operator's authority")
    );
    assert_eq!(granted.severity(), Severity::Ok);
}

// Row: a project enrolled into a team.
#[test]
fn enrolling_one_project_changes_only_that_projects_authority() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    // A second project in the same home, initialized the same way.
    let other = sandbox.dir.path().join("other");
    sandbox.init_repo(other.clone());
    h.execute(Operation::InitApply {
        root: other.clone(),
    });

    let answer = h.execute(Operation::Enroll {
        root: sandbox.project(),
        team: "acme".into(),
    });
    let Answer::Enrollment(change) = &answer else {
        panic!("expected an enrollment, got {answer:?}");
    };
    assert!(change.changed);
    assert_eq!(
        change.enrollment,
        Enrollment::Team {
            team: "acme".into()
        }
    );

    let enrolled = Manifest::read(&sandbox.project()).unwrap().unwrap();
    let untouched = Manifest::read(&other).unwrap().unwrap();
    assert_eq!(
        enrolled.project.enrollment,
        Enrollment::Team {
            team: "acme".into()
        }
    );
    assert_eq!(
        untouched.project.enrollment,
        Enrollment::Solo,
        "a sibling project's authority changed"
    );

    // And unenrolling puts it back, in that project only.
    h.execute(Operation::Unenroll {
        root: sandbox.project(),
    });
    assert_eq!(
        Manifest::read(&sandbox.project())
            .unwrap()
            .unwrap()
            .project
            .enrollment,
        Enrollment::Solo
    );
}

// Row: an enrolled project whose platform is unreachable, for a subject that
// requires shared approval.
#[test]
fn an_offline_team_project_cannot_bypass_a_required_shared_approval() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    let mut manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    manifest.project.enrollment = Enrollment::Team {
        team: "acme".into(),
    };
    manifest.project.shared_approval_required = vec!["003-x".into()];
    manifest
        .project
        .requirements
        .insert("model".into(), DEFER_TO_TEAM.into());
    manifest.write(&sandbox.project()).unwrap();

    // Recording a local approval is refused outright, rather than filed and
    // then ignored.
    let granted = h.execute(Operation::ApprovalGrant {
        root: sandbox.project(),
        subject: "003-x".into(),
        operator: "bart".into(),
        reason: "I say so".into(),
    });
    let Answer::Approval(outcome) = &granted else {
        panic!("expected an approval, got {granted:?}");
    };
    assert!(!outcome.recorded);
    assert!(
        outcome
            .refused
            .as_deref()
            .unwrap()
            .contains("cannot satisfy")
    );
    assert_eq!(outcome.eligibility.word(), "pending");
    assert_eq!(granted.severity(), Severity::Refused);

    // The project is still enrolled: an unreachable platform is not a
    // downgrade to solo.
    let after = Manifest::read(&sandbox.project()).unwrap().unwrap();
    assert_eq!(
        after.project.enrollment,
        Enrollment::Team {
            team: "acme".into()
        }
    );

    // A deferred key is unknown, and unknown is not success.
    let config = h.execute(Operation::ConfigShow {
        root: sandbox.project(),
        base_revision: "no-such-revision".into(),
        choices: statecraft_home::authority::RunChoices::none().choosing("model", "whatever"),
    });
    let Answer::Configuration(resolution) = &config else {
        panic!("expected a configuration, got {config:?}");
    };
    assert!(matches!(
        resolution.keys["model"],
        ConfigAnswer::Unknown { .. }
    ));

    // Work that needs no remote authority still proceeds.
    let local = h.execute(Operation::ApprovalGrant {
        root: sandbox.project(),
        subject: "004-y".into(),
        operator: "bart".into(),
        reason: "local only".into(),
    });
    let Answer::Approval(local) = &local else {
        panic!("expected an approval");
    };
    assert!(local.recorded);
    assert!(local.eligibility.eligible());

    // And when the authority does answer, the team's verdict is what decides.
    let stated = Stated {
        policy: None,
        approvals: vec![(
            "003-x".to_string(),
            SharedApproval::Granted {
                by: "reviewer".into(),
                at: "1970-01-01T00:00:00Z".into(),
            },
        )],
    };
    let online = harness!(sandbox, &producer, &corpus, &probe, &stated, &revisions);
    let answer = online.execute(Operation::ApprovalShow {
        root: sandbox.project(),
        subject: "003-x".into(),
    });
    let Answer::Approval(outcome) = &answer else {
        panic!("expected an approval");
    };
    assert!(outcome.eligibility.eligible());
    assert!(outcome.eligibility.describe().contains("team acme"));
}

// Row: a candidate diff that weakens a constraint in the declaration.
#[test]
fn a_candidate_cannot_weaken_the_policy_that_judges_it() {
    let sandbox = Sandbox::new();
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    h.execute(Operation::InitApply {
        root: sandbox.project(),
    });

    // The base revision requires an approved model.
    let mut manifest = Manifest::read(&sandbox.project()).unwrap().unwrap();
    manifest
        .project
        .requirements
        .insert("model".into(), "approved-a".into());
    manifest.write(&sandbox.project()).unwrap();
    let base = sandbox.commit("the trusted baseline");

    // The candidate rewrites the requirement to whatever it likes.
    let mut candidate = Manifest::read(&sandbox.project()).unwrap().unwrap();
    candidate
        .project
        .requirements
        .insert("model".into(), "anything-i-like".into());
    candidate.write(&sandbox.project()).unwrap();

    let answer = h.execute(Operation::ConfigShow {
        root: sandbox.project(),
        base_revision: base.clone(),
        choices: statecraft_home::authority::RunChoices::none(),
    });
    let Answer::Configuration(resolution) = &answer else {
        panic!("expected a configuration, got {answer:?}");
    };
    // GitRevision is not in play here: the suite reads through git itself.
    let read = statecraft_home::authority::GitRevision;
    let trusted =
        statecraft_home::authority::trusted(&sandbox.project(), &base, &read, &candidate.project);
    assert_eq!(
        trusted
            .project
            .requirements
            .get("model")
            .map(String::as_str),
        Some("approved-a"),
        "the base revision's answer is what applies"
    );
    let change = trusted
        .authority_change
        .expect("the candidate touched the authority set");
    assert!(change.keys.contains(&"model".to_string()));
    // The service call used a reader that knows nothing, so it reports the
    // absence of a baseline rather than trusting the candidate silently.
    assert!(resolution.authority_change.is_some());
}

// Row: an initialization interrupted after some steps, then repeated.
#[test]
fn an_interrupted_initialization_repeated_destroys_no_authored_content() {
    let sandbox = Sandbox::new();
    // A repository that already carries authored governance and a root file.
    sandbox.write(
        "specs/000-bootstrap/spec.md",
        "---\nid: \"000-bootstrap\"\n---\n\n# mine, authored by hand\n",
    );
    sandbox.write(project::ROOT_INSTRUCTIONS, "# our protocol\n");
    sandbox.write("spec-spine.toml", "# our own configuration\n");

    let producer = conforming_producer();
    let broken = support::StatedCorpus {
        ok: false,
        version: None,
    };
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();

    // First attempt: the corpus tool is not available, so the flow stops short
    // of a complete report and says so.
    let first = harness!(sandbox, &producer, &broken, &probe, &authority, &revisions);
    let answer = first.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);
    assert_eq!(report.outcome, Outcome::Partial);
    assert_ne!(
        report.outcome,
        Outcome::Complete,
        "partial work is never reported as complete"
    );
    assert_eq!(answer.severity(), Severity::Finding);

    // Nothing authored was destroyed, and the progress is recorded.
    assert!(
        sandbox
            .read("specs/000-bootstrap/spec.md")
            .unwrap()
            .contains("mine, authored by hand")
    );
    assert_eq!(
        sandbox.read("spec-spine.toml").unwrap(),
        "# our own configuration\n"
    );
    assert!(
        sandbox
            .read(project::ROOT_INSTRUCTIONS)
            .unwrap()
            .contains("# our protocol")
    );
    assert!(report.adopted.contains(&"spec-spine.toml".to_string()));
    let progress = statecraft_home::flow::Progress::read(&sandbox.project())
        .expect("the flow recorded how far it got");
    assert!(progress.last_completed.is_some());

    // Repeat with a working tool: still no authored file is rewritten.
    let corpus = support::StatedCorpus::fine();
    let second = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = second.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);
    assert_eq!(report.outcome, Outcome::Complete, "{:#?}", report.steps);
    assert!(
        sandbox
            .read("specs/000-bootstrap/spec.md")
            .unwrap()
            .contains("mine, authored by hand")
    );
    assert_eq!(
        sandbox.read("spec-spine.toml").unwrap(),
        "# our own configuration\n"
    );
    assert_eq!(
        sandbox
            .read(project::ROOT_INSTRUCTIONS)
            .unwrap()
            .matches(bridge::IMPORT_LINE)
            .count(),
        1
    );
}

// Row: a producer result carrying a path outside the contract set.
#[test]
fn a_producer_path_outside_the_contract_set_is_named_and_never_written() {
    let sandbox = Sandbox::new();
    let producer = statecraft_home::producer::Recorded {
        json: serde_json::json!({
            "files": [
                { "relPath": "spec-spine.toml", "contents": "[layout]\n" },
                { "relPath": "AGENTS.md", "contents": "# generated by a producer\n" },
                { "relPath": ".claude/rules/orchestrator-rules.md", "contents": "rules\n" },
                { "relPath": "Makefile", "contents": "all:\n" }
            ]
        })
        .to_string(),
        identity: statecraft_home::producer::Identity {
            name: "out-of-contract".into(),
            version: "0".into(),
        },
    };
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);
    let answer = h.execute(Operation::InitApply {
        root: sandbox.project(),
    });
    let report = init_report(&answer);

    let conformance = report.conformance.clone().expect("conformance is reported");
    assert!(!conformance.conforming);
    assert_eq!(
        conformance.out_of_contract,
        [
            "AGENTS.md",
            ".claude/rules/orchestrator-rules.md",
            "Makefile"
        ]
    );
    assert!(!sandbox.exists(".claude/rules/orchestrator-rules.md"));
    assert!(!sandbox.exists("Makefile"));
    // The in-contract path still landed, and the outcome is not complete.
    assert!(sandbox.exists("spec-spine.toml"));
    assert_eq!(report.outcome, Outcome::Partial);
    // The producer's own AGENTS.md is recognized, not written: the root file
    // exists only because the bridge created it, and it holds the import.
    assert_eq!(
        sandbox.read(project::ROOT_INSTRUCTIONS).unwrap(),
        format!("{}\n", bridge::IMPORT_LINE)
    );
}

// Row: both `.derived/` and `.statecraft/derived/` present.
#[test]
fn both_derived_destinations_present_refuses_and_moves_nothing() {
    let sandbox = Sandbox::new();
    sandbox.write(".derived/spec-registry/a.json", "old");
    sandbox.write(".statecraft/derived/spec-registry/a.json", "new");
    let producer = conforming_producer();
    let corpus = support::StatedCorpus::fine();
    let probe = StatedProbe::default();
    let authority = Unreachable::default();
    let revisions = StaticRevision::default();
    let h = harness!(sandbox, &producer, &corpus, &probe, &authority, &revisions);

    let answer = h.execute(Operation::MigrateApply {
        root: sandbox.project(),
    });
    assert_eq!(answer.severity(), Severity::Refused);
    let Answer::Migrate(outcome) = &answer else {
        panic!("expected a relocation, got {answer:?}");
    };
    match &outcome.verdict {
        derived::Verdict::Refused { reason } => {
            assert!(reason.contains(".derived"));
            assert!(reason.contains(".statecraft/derived"));
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert!(outcome.moved.is_empty());
    assert_eq!(
        sandbox.read(".derived/spec-registry/a.json").unwrap(),
        "old"
    );
    assert_eq!(
        sandbox
            .read(".statecraft/derived/spec-registry/a.json")
            .unwrap(),
        "new"
    );
}

// Row: a declared value that is an absolute path or begins with `~`.
#[test]
fn a_declared_absolute_path_is_refused_at_write_time_with_the_key_named() {
    let sandbox = Sandbox::new();
    let mut manifest = Manifest::new(Pins {
        product: "0".into(),
        spec_spine: "0".into(),
        adapters: Default::default(),
    });
    manifest
        .project
        .requirements
        .insert("spec-spine".into(), "/Users/someone/bin/spec-spine".into());
    manifest
        .project
        .overrides
        .insert("harness".into(), "~/.statecraft/harness".into());

    let error = manifest.write(&sandbox.project()).unwrap_err();
    let rendered = error.to_string();
    assert!(rendered.contains("spec-spine"), "{rendered}");
    assert!(rendered.contains("harness"), "{rendered}");
    assert!(rendered.contains("not portable"), "{rendered}");
    assert!(
        !sandbox.exists(project::DECLARATION),
        "a declaration nobody else can satisfy was written anyway"
    );
}
