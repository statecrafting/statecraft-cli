//! Spec 002 section 3.35, per-path ownership transfer, against the library.
//!
//! Every test works in a temporary fixture repository with a test adapter, and
//! every refusal is checked for the half an error value cannot carry: that no
//! byte of the repository changed. The same behaviors are exercised through
//! the built binary in `statecraft-cli`'s `tests/ownership_transfer.rs`.

#![cfg(unix)]

use statecraft_environment::adapter::{Declaration, ManagedFile, StaticProbe};
use statecraft_environment::apply::{Outcome as EnvOutcome, apply as env_apply, remove};
use statecraft_environment::claimant::ForeignClaims;
use statecraft_environment::digest::digest_bytes;
use statecraft_environment::manifest::{
    Class, MANIFEST_PATH, Manifest, Modification, ModificationKind, Pins,
};
use statecraft_environment::plan::{Withholding, plan as env_plan};
use statecraft_environment::time::FixedClock;
use statecraft_environment::transfer::{
    Act, Context, Outcome, Ownership, RefusalKind, TransferError, apply, plan, revert,
};
use std::collections::BTreeMap;
use std::path::Path;

const HARNESS: &str = "test-harness";
const ADAPTER: &str = "test-adapter";
const OWNED: &str = ".tool/instructions.md";
const POINTER: &str = "CLAUDE.md";
const PRODUCER: &str = "spec-spine-core@0.23.0";

use Ownership::{Adopted, Managed, User};

struct Fixture {
    dir: tempfile::TempDir,
    declarations: Vec<Declaration>,
    foreign: ForeignClaims,
    probe: StaticProbe,
}

impl Fixture {
    /// A repository with an empty manifest and one installed test adapter.
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        Manifest::new(Pins {
            product: "0.0.0".into(),
            spec_spine: "0.23.0".into(),
            adapters: BTreeMap::new(),
        })
        .write(dir.path())
        .unwrap();
        Self {
            dir,
            declarations: vec![Declaration {
                name: ADAPTER.into(),
                harness: HARNESS.into(),
                version: "1".into(),
                files: vec![
                    ManagedFile::owned(OWNED, b"managed bytes\n".to_vec()),
                    ManagedFile::pointer(POINTER, b"@.tool/instructions.md\n".to_vec()),
                ],
                unexpressible: vec![],
                prerequisites: vec![],
            }],
            foreign: ForeignClaims::none(),
            probe: StaticProbe::new().with_harness(HARNESS),
        }
    }

    /// The same, with the adapter's harness absent: it declares its paths
    /// and claims none of them.
    fn not_claiming() -> Self {
        let mut f = Self::new();
        f.probe = StaticProbe::new();
        f
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn ctx(&self) -> Context<'_> {
        Context {
            root: self.root(),
            producer: PRODUCER,
            declarations: &self.declarations,
            probe: &self.probe,
            foreign: &self.foreign,
        }
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let at = self.root().join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, bytes).unwrap();
    }

    fn read(&self, path: &str) -> Vec<u8> {
        std::fs::read(self.root().join(path)).unwrap()
    }

    fn manifest(&self) -> Manifest {
        Manifest::read(self.root()).unwrap().unwrap()
    }

    fn env_apply(&self) -> EnvOutcome {
        let mut m = self.manifest();
        env_apply(
            self.root(),
            &mut m,
            &self.declarations,
            &StaticProbe::new().with_harness(HARNESS),
            &self.foreign,
            &FixedClock(1_700_000_000),
        )
        .unwrap()
    }

    fn plan_id(&self, path: &str, from: Ownership, to: Ownership) -> String {
        plan(&self.ctx(), path, from, to).unwrap().plan_id
    }

    fn apply(&self, path: &str, from: Ownership, to: Ownership, id: &str) -> Outcome {
        apply(&self.ctx(), path, from, to, id, &act(), &clock()).unwrap()
    }

    /// Plan and apply in one step.
    fn transfer(&self, path: &str, from: Ownership, to: Ownership) -> Outcome {
        let id = self.plan_id(path, from, to);
        self.apply(path, from, to, &id)
    }

    fn revert(&self, id: &str) -> Result<Outcome, TransferError> {
        revert(&self.ctx(), id, &act(), &clock())
    }

    /// Every byte under the root, including the manifest.
    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        walk(self.root(), self.root(), &mut out);
        out
    }
}

fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    for entry in std::fs::read_dir(at).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let rel = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        // Runtime state (the manifest lock file) is gitignored and not a byte
        // of the committed repository.
        if rel == ".statecraft/state" {
            continue;
        }
        let kind = entry.file_type().unwrap();
        if kind.is_symlink() {
            let target = std::fs::read_link(&path).unwrap();
            out.insert(rel, format!("-> {}", target.display()).into_bytes());
        } else if kind.is_dir() {
            out.insert(format!("{rel}/"), Vec::new());
            walk(root, &path, out);
        } else {
            out.insert(rel, std::fs::read(&path).unwrap());
        }
    }
}

fn act() -> Act<'static> {
    Act {
        operator: "bart",
        reason: "the file is ours now",
    }
}

fn clock() -> FixedClock {
    FixedClock(1_800_000_000)
}

fn refusal<T: std::fmt::Debug>(result: Result<T, TransferError>) -> RefusalKind {
    match result {
        Err(TransferError::Refused(r)) => r.kind,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Assert a refusal of `kind` that left every byte as it was.
fn refused_unchanged<T: std::fmt::Debug>(
    f: &Fixture,
    kind: RefusalKind,
    op: impl FnOnce() -> Result<T, TransferError>,
) {
    let before = f.snapshot();
    assert_eq!(refusal(op()), kind);
    assert_eq!(f.snapshot(), before, "a refusal changed a byte");
}

// Acceptance: `user` to `adopted` and back.
#[test]
fn user_to_adopted_and_back_changes_ownership_and_never_the_file() {
    let f = Fixture::new();
    f.write("docs/policy.md", b"the user's policy\n");
    let before = f.snapshot();

    let p = plan(&f.ctx(), "docs/policy.md", User, Adopted).unwrap();
    assert_eq!(f.snapshot(), before, "plan writes nothing");
    assert_eq!(p.current.class, User);
    assert_eq!(p.digest, digest_bytes(b"the user's policy\n"));
    assert_eq!(p.bytes, 18);
    assert_eq!(p.producer, PRODUCER);
    assert_eq!(p.resulting.as_ref().unwrap().class, Class::Adopted);
    assert_eq!(p.identity.len(), 64);
    assert!(p.plan_id.starts_with("pi1-user-adopted-"), "{}", p.plan_id);
    assert!(p.plan_id.ends_with(&p.identity));

    let applied = f.apply("docs/policy.md", User, Adopted, &p.plan_id);
    assert_eq!(applied.word(), "applied");
    let record = applied.record().clone();
    let m = f.manifest();
    let entry = m.entry("docs/policy.md").unwrap();
    assert_eq!(entry.class, Class::Adopted);
    assert_eq!(entry.digest, p.digest);
    assert!(
        entry.transfer.is_none(),
        "rule 1: an adoption names no claimant"
    );
    assert_eq!(m.transfers, vec![record.clone()]);
    assert_eq!(record.from, User);
    assert_eq!(record.to, Adopted);
    assert_eq!(record.digest, p.digest);
    assert_eq!(record.bytes, 18);
    assert_eq!(record.producer, PRODUCER);
    assert_eq!(record.operator, "bart");
    assert_eq!(record.reason, "the file is ours now");
    assert_eq!(record.at, "2027-01-15T08:00:00Z");
    assert_eq!(record.manifest_before, p.manifest_digest);
    assert_eq!(record.reverts, None);
    assert_eq!(f.read("docs/policy.md"), b"the user's policy\n");

    let reverted = f.revert(&record.id).unwrap();
    assert_eq!(reverted.word(), "reverted");
    let m = f.manifest();
    assert!(!m.records("docs/policy.md"));
    assert_eq!(m.transfers.len(), 2, "the reversed record is kept");
    assert_eq!(m.transfers[0], record);
    assert_eq!(m.transfers[1].reverts.as_deref(), Some(record.id.as_str()));
    assert_eq!((m.transfers[1].from, m.transfers[1].to), (Adopted, User));
    assert_eq!(f.read("docs/policy.md"), b"the user's policy\n");
}

// Acceptance: `user` to `managed` for an adapter path `env apply` withheld as
// `foreign`, after which `env apply` writes it; and the reversal, after which
// it is withheld again.
#[test]
fn a_foreign_adapter_path_transferred_to_managed_is_written_and_withheld_again_after_reversal() {
    let f = Fixture::new();
    f.write(OWNED, b"the user's copy\n");
    match f.env_apply() {
        EnvOutcome::Partial { withheld, .. } => {
            let w = withheld.iter().find(|w| w.path == OWNED).unwrap();
            assert!(matches!(w.reason, Withholding::Foreign { .. }));
        }
        other => panic!("expected partial, got {other:?}"),
    }
    assert_eq!(f.read(OWNED), b"the user's copy\n");

    let record = f.transfer(OWNED, User, Managed).record().clone();
    let entry = f.manifest().entry(OWNED).unwrap().clone();
    assert_eq!(entry.class, Class::Managed);
    assert_eq!(entry.source.identity, ADAPTER);
    let t = entry.transfer.unwrap();
    assert_eq!(t.digest_at_transfer, digest_bytes(b"the user's copy\n"));
    assert_eq!(t.evaluated_against.as_deref(), Some(PRODUCER));
    assert_eq!(
        f.read(OWNED),
        b"the user's copy\n",
        "the transfer wrote nothing"
    );

    assert!(matches!(f.env_apply(), EnvOutcome::Applied { .. }));
    assert_eq!(f.read(OWNED), b"managed bytes\n", "env apply now writes it");
    assert!(
        f.manifest().entry(OWNED).unwrap().transfer.is_some(),
        "the rewrite keeps the transfer"
    );

    // Rule 6: a rewrite this product recorded is not an intervening change.
    let reverted = f.revert(&record.id).unwrap();
    assert_eq!(reverted.record().digest, digest_bytes(b"managed bytes\n"));
    assert!(!f.manifest().records(OWNED));
    assert_eq!(
        f.read(OWNED),
        b"managed bytes\n",
        "reversal changes ownership, never content"
    );

    let again = env_plan(
        f.root(),
        Some(&f.manifest()),
        &f.declarations,
        &StaticProbe::new().with_harness(HARNESS),
        &f.foreign,
    )
    .unwrap();
    let w = again.withheld.iter().find(|w| w.path == OWNED).unwrap();
    assert!(matches!(w.reason, Withholding::Foreign { .. }));
}

// Acceptance: `managed` to `user`, after which `env remove` leaves the file.
#[test]
fn a_managed_path_released_to_user_is_left_by_env_remove() {
    let f = Fixture::new();
    assert!(matches!(f.env_apply(), EnvOutcome::Applied { .. }));
    assert_eq!(f.read(OWNED), b"managed bytes\n");

    f.transfer(OWNED, Managed, User);
    assert!(!f.manifest().records(OWNED));
    remove(f.root(), &clock()).unwrap();
    assert_eq!(
        f.read(OWNED),
        b"managed bytes\n",
        "the file stays, byte for byte"
    );
    assert!(
        !f.root().join(POINTER).exists(),
        "the pointer was still managed"
    );
}

// Rule 2: a pointer this product wrote is managed and may be released.
#[test]
fn a_managed_pointer_file_may_be_released_to_user() {
    let f = Fixture::new();
    f.env_apply();
    let out = f.transfer(POINTER, Managed, User);
    assert_eq!(out.word(), "applied");
    assert!(!f.manifest().records(POINTER));
}

// Rule 2: its reversal would make an instruction file managed, so it refuses.
#[test]
fn reversing_the_release_of_a_pointer_file_refuses_because_it_is_an_instruction_file() {
    let f = Fixture::new();
    f.env_apply();
    let record = f.transfer(POINTER, Managed, User).record().clone();
    refused_unchanged(&f, RefusalKind::InstructionFile, || f.revert(&record.id));
}

// Negative: a stale plan after the file changed.
#[test]
fn a_plan_is_stale_after_the_file_changes() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    f.write("notes.md", b"two\n");
    refused_unchanged(&f, RefusalKind::StalePlan, || {
        apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock())
    });
}

// Negative: a stale plan after the manifest changed.
#[test]
fn a_plan_is_stale_after_the_manifest_changes() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    f.write("other.md", b"other\n");
    let id = f.plan_id("notes.md", User, Adopted);
    f.transfer("other.md", User, Adopted);
    let err = apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock()).unwrap_err();
    match err {
        TransferError::Refused(r) => {
            assert_eq!(r.kind, RefusalKind::StalePlan);
            assert!(r.detail.contains("current plan identity"), "{}", r.detail);
        }
        other => panic!("{other:?}"),
    }
}

// Negative: a stale plan after the class changed.
#[test]
fn a_plan_is_refused_after_the_class_changes() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    f.transfer("notes.md", User, Adopted);
    f.transfer("notes.md", Adopted, User);
    // The class is `user` again, but the manifest moved: stale.
    refused_unchanged(&f, RefusalKind::StalePlan, || {
        apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock())
    });
    // A plan made while the path was `user` names a class that is not the
    // path's once another transfer made it `managed`.
    f.write(OWNED, b"the user's copy\n");
    let adopt = f.plan_id(OWNED, User, Adopted);
    f.transfer(OWNED, User, Managed);
    refused_unchanged(&f, RefusalKind::ClassMismatch, || {
        apply(&f.ctx(), OWNED, User, Adopted, &adopt, &act(), &clock())
    });
}

// Negative: a named class that is not the path's.
#[test]
fn a_named_class_that_is_not_the_paths_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    refused_unchanged(&f, RefusalKind::ClassMismatch, || {
        plan(&f.ctx(), "notes.md", Adopted, User)
    });
    refused_unchanged(&f, RefusalKind::ClassMismatch, || {
        apply(&f.ctx(), "notes.md", Managed, User, "x", &act(), &clock())
    });
}

// Negative: `user` to `managed` for a path no adapter declares.
#[test]
fn user_to_managed_without_an_adapter_source_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    refused_unchanged(&f, RefusalKind::NoSource, || {
        plan(&f.ctx(), "notes.md", User, Managed)
    });
}

// Rule 1: moves the table does not admit.
#[test]
fn a_move_the_table_does_not_admit_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    for (from, to) in [
        (Adopted, Managed),
        (Managed, Adopted),
        (User, User),
        (Adopted, Adopted),
    ] {
        refused_unchanged(&f, RefusalKind::MoveNotAdmitted, || {
            plan(&f.ctx(), "notes.md", from, to)
        });
    }
}

// Negative: a root `AGENTS.md`, and the rest of rule 2's closed list.
#[test]
fn a_user_instruction_file_cannot_be_adopted_or_managed() {
    let f = Fixture::new();
    for path in [
        "AGENTS.md",
        "sub/dir/CLAUDE.md",
        "GEMINI.md",
        ".cursorrules",
        "pkg/.windsurfrules",
        ".github/copilot-instructions.md",
        "nested/.github/copilot-instructions.md",
        // A case-insensitive filesystem opens the listed file by this name.
        "agents.md",
    ] {
        f.write(path, b"the user's instructions\n");
        for to in [Adopted, Managed] {
            refused_unchanged(&f, RefusalKind::InstructionFile, || {
                plan(&f.ctx(), path, User, to)
            });
        }
    }
    // Not on the list: an ordinary file whose name only resembles one.
    f.write("docs/AGENTS.md.bak", b"x");
    assert!(plan(&f.ctx(), "docs/AGENTS.md.bak", User, Adopted).is_ok());
    f.write("x.github/copilot-instructions.md", b"x");
    assert!(plan(&f.ctx(), "x.github/copilot-instructions.md", User, Adopted).is_ok());
}

// Rule 2: the instruction bridge is a modification, not an entry.
#[test]
fn a_path_carrying_a_tracked_modification_is_not_transferable() {
    let f = Fixture::new();
    f.write("NOTES.md", b"@.statecraft/AGENTS.md\nmine\n");
    let mut m = f.manifest();
    m.upsert_modification(Modification {
        path: "NOTES.md".into(),
        kind: ModificationKind::ImportBridge,
        line: "@.statecraft/AGENTS.md".into(),
        digest_before: None,
        digest_after: "d".into(),
        written_at: "1970-01-01T00:00:00Z".into(),
    });
    m.write(f.root()).unwrap();
    refused_unchanged(&f, RefusalKind::Modification, || {
        plan(&f.ctx(), "NOTES.md", User, Adopted)
    });
}

// Negative: rule 3's paths.
#[test]
fn paths_rule_3_forbids_are_refused_by_rule() {
    let f = Fixture::new();
    f.write("real/file.md", b"x");
    f.write(".git/config", b"x");
    f.write("sub/.git/HEAD", b"x");
    std::fs::create_dir_all(f.root().join("a-directory")).unwrap();
    std::os::unix::fs::symlink(f.root().join("real/file.md"), f.root().join("link.md")).unwrap();
    std::os::unix::fs::symlink(f.root().join("real"), f.root().join("linkdir")).unwrap();

    let cases: [(&str, RefusalKind); 16] = [
        ("", RefusalKind::NotRelative),
        ("/etc/passwd", RefusalKind::NotRelative),
        ("real\\file.md", RefusalKind::NotRelative),
        ("../escape.md", RefusalKind::Escaping),
        ("real/../real/file.md", RefusalKind::Escaping),
        ("./real/file.md", RefusalKind::Escaping),
        ("real//file.md", RefusalKind::Escaping),
        ("real/", RefusalKind::Escaping),
        (MANIFEST_PATH, RefusalKind::Protected),
        (".Statecraft/environment.json", RefusalKind::Protected),
        (".git/config", RefusalKind::Protected),
        ("sub/.git/HEAD", RefusalKind::Protected),
        ("link.md", RefusalKind::SymbolicLink),
        ("linkdir/file.md", RefusalKind::SymbolicLink),
        ("a-directory", RefusalKind::Directory),
        ("absent.md", RefusalKind::Missing),
    ];
    for (path, kind) in cases {
        let before = f.snapshot();
        let got = refusal(plan(&f.ctx(), path, User, Adopted));
        assert_eq!(got, kind, "`{path}`");
        let got = refusal(apply(&f.ctx(), path, User, Adopted, "x", &act(), &clock()));
        assert_eq!(got, kind, "`{path}` through apply");
        assert_eq!(f.snapshot(), before, "`{path}`");
    }
}

// Negative: a reversal after an intervening edit.
#[test]
fn a_reversal_after_an_unrecorded_edit_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let record = f.transfer("notes.md", User, Adopted).record().clone();
    f.write("notes.md", b"edited\n");
    refused_unchanged(&f, RefusalKind::UnrecordedChange, || f.revert(&record.id));

    // And after a move to `user`, the digest the transfer observed decides.
    let g = Fixture::new();
    g.env_apply();
    let record = g.transfer(OWNED, Managed, User).record().clone();
    g.write(OWNED, b"the user edited it\n");
    refused_unchanged(&g, RefusalKind::UnrecordedChange, || g.revert(&record.id));
}

// Negative: a reversal after a later transfer.
#[test]
fn a_reversal_after_a_later_transfer_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let first = f.transfer("notes.md", User, Adopted).record().clone();
    f.transfer("notes.md", Adopted, User);
    refused_unchanged(&f, RefusalKind::LaterTransfer, || f.revert(&first.id));
}

// Rule 6: a reversal of a revert is the latest transfer, and is admitted.
#[test]
fn the_latest_record_may_itself_be_a_reversal_and_is_reversible() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let first = f.transfer("notes.md", User, Adopted).record().clone();
    let second = f.revert(&first.id).unwrap().record().clone();
    let third = f.revert(&second.id).unwrap();
    assert_eq!((third.record().from, third.record().to), (User, Adopted));
    assert!(f.manifest().records("notes.md"));
}

#[test]
fn a_reversal_of_a_missing_file_or_an_unknown_record_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let record = f.transfer("notes.md", User, Adopted).record().clone();
    refused_unchanged(&f, RefusalKind::UnknownTransfer, || f.revert("no-such-id"));
    std::fs::remove_file(f.root().join("notes.md")).unwrap();
    refused_unchanged(&f, RefusalKind::Missing, || f.revert(&record.id));
}

// Negative: a repeated request reported `already-satisfied`, nothing written.
#[test]
fn a_repeated_request_is_already_satisfied_whatever_plan_identity_is_given() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    let first = f.apply("notes.md", User, Adopted, &id);
    let before = f.snapshot();
    for given in [id.as_str(), "not-a-plan-at-all"] {
        let again = f.apply("notes.md", User, Adopted, given);
        assert_eq!(again.word(), "already-satisfied");
        assert_eq!(again.record(), first.record());
        assert_eq!(f.snapshot(), before, "nothing written");
    }
    // Not satisfied once the file moved on: then the class decides.
    f.write("notes.md", b"edited\n");
    refused_unchanged(&f, RefusalKind::ClassMismatch, || {
        apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock())
    });
}

// Rule 5: a journal that disagrees with the entries.
#[test]
fn a_journal_that_disagrees_with_the_entries_is_reported_by_plan_and_refused_by_apply_and_revert() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    f.write("other.md", b"two\n");
    let record = f.transfer("notes.md", User, Adopted).record().clone();
    // Someone removes the entry by hand; the journal still says `adopted`.
    let mut m = f.manifest();
    m.remove("notes.md");
    m.write(f.root()).unwrap();

    let p = plan(&f.ctx(), "other.md", User, Adopted).unwrap();
    assert_eq!(p.journal_disagreements.len(), 1, "{p:?}");
    assert!(p.journal_disagreements[0].contains("notes.md"));
    refused_unchanged(&f, RefusalKind::JournalDisagrees, || {
        apply(
            &f.ctx(),
            "other.md",
            User,
            Adopted,
            &p.plan_id,
            &act(),
            &clock(),
        )
    });
    refused_unchanged(&f, RefusalKind::JournalDisagrees, || f.revert(&record.id));
}

// Rule 5's two recorded differences that are not disagreements.
#[test]
fn env_remove_and_a_fresh_env_apply_do_not_make_the_journal_disagree() {
    let f = Fixture::new();
    f.write(OWNED, b"the user's copy\n");
    f.transfer(OWNED, User, Managed);
    f.env_apply();
    remove(f.root(), &clock()).unwrap();
    assert!(!f.root().join(OWNED).exists());
    assert!(!f.manifest().records(OWNED));
    f.write("notes.md", b"one\n");
    let p = plan(&f.ctx(), "notes.md", User, Adopted).unwrap();
    assert!(p.journal_disagreements.is_empty(), "{p:?}");
    // Restoring a file at the path afterwards is still not a disagreement.
    f.write(OWNED, b"restored by the user\n");
    let p = plan(&f.ctx(), "notes.md", User, Adopted).unwrap();
    assert!(p.journal_disagreements.is_empty(), "{p:?}");
    assert_eq!(
        f.apply("notes.md", User, Adopted, &p.plan_id).word(),
        "applied"
    );

    let g = Fixture::new();
    g.env_apply();
    g.transfer(OWNED, Managed, User);
    std::fs::remove_file(g.root().join(OWNED)).unwrap();
    g.env_apply();
    assert_eq!(g.manifest().entry(OWNED).unwrap().class, Class::Managed);
    g.write("notes.md", b"one\n");
    let p = plan(&g.ctx(), "notes.md", User, Adopted).unwrap();
    assert!(p.journal_disagreements.is_empty(), "{p:?}");
}

// Compatibility: a manifest written before section 3.35.
#[test]
fn a_manifest_with_no_journal_and_a_legacy_transfer_reads_as_before_and_is_never_rewritten() {
    let f = Fixture::new();
    f.write(OWNED, b"legacy\n");
    f.write("notes.md", b"one\n");
    let legacy = format!(
        r#"{{
  "version": 2,
  "pins": {{ "product": "0.0.0", "spec_spine": "0.20.0" }},
  "entries": [
    {{
      "path": "{OWNED}",
      "class": "managed",
      "source": {{ "kind": "adapter", "identity": "{ADAPTER}" }},
      "digest": "{}",
      "bytes": 7,
      "written_at": "2026-09-01T00:00:00Z",
      "transfer": {{
        "from": {{ "kind": "package", "name": "kit", "revision": "0.18.0" }},
        "digest_at_transfer": "{}"
      }}
    }}
  ]
}}
"#,
        digest_bytes(b"legacy\n"),
        digest_bytes(b"legacy\n")
    );
    f.write(MANIFEST_PATH, legacy.as_bytes());
    let m = f.manifest();
    assert!(m.transfers.is_empty(), "no journal reads as no transfers");
    let legacy_entry = m.entry(OWNED).unwrap().clone();

    let p = plan(&f.ctx(), "notes.md", User, Adopted).unwrap();
    assert_eq!(p.recorded_without_journal, vec![OWNED.to_string()]);
    assert!(p.journal_disagreements.is_empty());
    f.apply("notes.md", User, Adopted, &p.plan_id);
    assert_eq!(
        f.manifest().entry(OWNED).unwrap(),
        &legacy_entry,
        "never rewritten"
    );
}

#[test]
fn a_repository_with_no_manifest_is_refused() {
    let f = Fixture::new();
    std::fs::remove_file(f.root().join(MANIFEST_PATH)).unwrap();
    f.write("notes.md", b"one\n");
    refused_unchanged(&f, RefusalKind::NoManifest, || {
        plan(&f.ctx(), "notes.md", User, Adopted)
    });
    refused_unchanged(&f, RefusalKind::NoManifest, || {
        apply(&f.ctx(), "notes.md", User, Adopted, "x", &act(), &clock())
    });
}

#[test]
fn the_operator_is_recorded_as_supplied_and_an_empty_one_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    for (operator, reason) in [("", "why"), ("  ", "why"), ("bart", ""), ("bart", " ")] {
        refused_unchanged(&f, RefusalKind::MissingOperatorOrReason, || {
            apply(
                &f.ctx(),
                "notes.md",
                User,
                Adopted,
                &id,
                &Act { operator, reason },
                &clock(),
            )
        });
    }
    let out = apply(
        &f.ctx(),
        "notes.md",
        User,
        Adopted,
        &id,
        &Act {
            operator: " Bart K. ",
            reason: "why not",
        },
        &clock(),
    )
    .unwrap();
    assert_eq!(out.record().operator, " Bart K. ");
    assert_eq!(out.record().operator_provenance, "operator-supplied");
}

// Concurrency: another holder of the lock refuses the transfer, writing nothing.
#[test]
fn a_transfer_while_another_process_holds_the_manifest_lock_is_refused() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    // Held by another thread, as another process would hold it: within one
    // thread the lock is reentrant.
    let (taken, release) = (std::sync::mpsc::channel(), std::sync::mpsc::channel::<()>());
    let root = f.root().to_path_buf();
    let holder = std::thread::spawn(move || {
        let _held =
            statecraft_environment::manifest::lock(&root, std::time::Duration::ZERO).unwrap();
        taken.0.send(()).unwrap();
        release.1.recv().unwrap();
    });
    taken.1.recv().unwrap();
    refused_unchanged(&f, RefusalKind::Busy, || {
        apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock())
    });
    release.0.send(()).unwrap();
    holder.join().unwrap();
    assert_eq!(f.apply("notes.md", User, Adopted, &id).word(), "applied");
}

// Concurrency: many applies of one plan at once journal it exactly once.
#[test]
fn concurrent_applies_of_one_plan_journal_it_exactly_once() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    let words: Vec<String> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                s.spawn(|| {
                    match apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock()) {
                        Ok(o) => o.word().to_string(),
                        Err(TransferError::Refused(r)) => format!("{:?}", r.kind),
                        Err(e) => panic!("{e}"),
                    }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    assert_eq!(
        words.iter().filter(|w| *w == "applied").count(),
        1,
        "{words:?}"
    );
    for w in &words {
        assert!(
            w == "applied" || w == "already-satisfied" || w == "Busy",
            "{words:?}"
        );
    }
    assert_eq!(f.manifest().transfers.len(), 1);
}

// Durability: a write that cannot be committed leaves the old manifest whole.
#[test]
fn a_manifest_write_that_fails_leaves_the_old_manifest_and_no_temporary_file() {
    use std::os::unix::fs::PermissionsExt as _;
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    let id = f.plan_id("notes.md", User, Adopted);
    let area = f.root().join(".statecraft");
    let before = f.snapshot();
    std::fs::set_permissions(&area, std::fs::Permissions::from_mode(0o555)).unwrap();
    // A superuser writes through the mode, and then there is nothing to test.
    let probe = area.join(".probe");
    if std::fs::write(&probe, b"").is_ok() {
        let _ = std::fs::remove_file(&probe);
        std::fs::set_permissions(&area, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let result = apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock());
    std::fs::set_permissions(&area, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(result, Err(TransferError::Manifest(_))),
        "{result:?}"
    );
    assert_eq!(f.snapshot(), before, "old manifest, no temporary file");
}

// Rule 1: `managed` only where the adapter claims its paths on this machine.
#[test]
fn user_to_managed_is_refused_when_the_declaring_adapter_does_not_claim_here() {
    let f = Fixture::not_claiming();
    f.write(OWNED, b"the user's copy\n");
    refused_unchanged(&f, RefusalKind::AdapterNotClaiming, || {
        plan(&f.ctx(), OWNED, User, Managed)
    });
    // Adoption does not need a source, and is still admitted.
    assert!(plan(&f.ctx(), OWNED, User, Adopted).is_ok());
}

// Rule 4: a stale plan names which input changed.
#[test]
fn a_stale_plan_names_exactly_which_input_changed() {
    fn changed(r: Result<Outcome, TransferError>) -> Vec<String> {
        match r {
            Err(TransferError::Refused(r)) => {
                assert_eq!(r.kind, RefusalKind::StalePlan);
                r.changed
            }
            other => panic!("{other:?}"),
        }
    }
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    f.write("other.md", b"other\n");
    f.write(OWNED, b"x\n");
    let go = |path: &str, from, to, id: &str| apply(&f.ctx(), path, from, to, id, &act(), &clock());

    let id = f.plan_id("notes.md", User, Adopted);
    f.write("notes.md", b"two\n");
    assert_eq!(changed(go("notes.md", User, Adopted, &id)), ["file"]);

    let id = f.plan_id("notes.md", User, Adopted);
    f.transfer("other.md", User, Adopted);
    assert_eq!(changed(go("notes.md", User, Adopted, &id)), ["manifest"]);

    let other = f.plan_id("other.md", Adopted, User);
    assert_eq!(
        changed(go("notes.md", User, Adopted, &other)),
        ["classes", "path", "file"]
    );

    let managed = f.plan_id(OWNED, User, Managed);
    assert_eq!(changed(go(OWNED, User, Adopted, &managed)), ["classes"]);

    assert_eq!(
        changed(go("notes.md", User, Adopted, "garbage")),
        ["identity"]
    );
    let id = f.plan_id("notes.md", User, Adopted);
    let forged = format!(
        "{}{}",
        &id[..id.len() - 1],
        if id.ends_with('0') { '1' } else { '0' }
    );
    assert_eq!(
        changed(go("notes.md", User, Adopted, &forged)),
        ["identity"]
    );
}

// Rule 3: one file is never recorded under two spellings.
#[test]
fn a_spelling_the_directory_does_not_list_is_refused() {
    let f = Fixture::new();
    f.write("Notes.md", b"one\n");
    // Composed on the way in; a normalization-insensitive volume opens the
    // decomposed name for it.
    f.write("cafe\u{301}.md", b"two\n");
    for (asked, stored) in [("notes.md", "Notes.md"), ("caf\u{e9}.md", "cafe\u{301}.md")] {
        let opens = f.root().join(asked).exists();
        let kind = if opens {
            RefusalKind::Spelling
        } else {
            RefusalKind::Missing
        };
        refused_unchanged(&f, kind, || plan(&f.ctx(), asked, User, Adopted));
        // The spelling the directory lists is accepted.
        assert!(plan(&f.ctx(), stored, User, Adopted).is_ok(), "{stored}");
    }
}

#[test]
fn a_second_name_for_a_recorded_file_is_refused_as_an_alias() {
    let f = Fixture::new();
    f.write("a.md", b"one\n");
    f.transfer("a.md", User, Adopted);
    std::fs::hard_link(f.root().join("a.md"), f.root().join("b.md")).unwrap();
    refused_unchanged(&f, RefusalKind::Alias, || {
        plan(&f.ctx(), "b.md", User, Adopted)
    });
}

// Rule 3: `.statecraft` is protected at any depth.
#[test]
fn a_nested_statecraft_component_is_protected() {
    let f = Fixture::new();
    f.write("sub/.statecraft/environment.json", b"{}");
    refused_unchanged(&f, RefusalKind::Protected, || {
        plan(&f.ctx(), "sub/.statecraft/environment.json", User, Adopted)
    });
}

// The one-writer guarantee: an `env apply` and a transfer apply racing each
// other never lose anything either reported as done.
#[test]
fn concurrent_env_apply_and_transfer_apply_lose_nothing_reported_as_done() {
    for round in 0..20 {
        let f = Fixture::new();
        f.write("notes.md", b"one\n");
        let id = f.plan_id("notes.md", User, Adopted);
        let (transfer, env) = std::thread::scope(|s| {
            let t = s.spawn(|| apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock()));
            let e = s.spawn(|| {
                statecraft_environment::apply::apply_current(
                    f.root(),
                    &f.declarations,
                    &f.probe,
                    &f.foreign,
                    &FixedClock(1_700_000_000),
                    || f.manifest().pins,
                )
            });
            (t.join().unwrap(), e.join().unwrap())
        });
        let m = f.manifest();
        match transfer {
            Ok(o) => {
                assert_eq!(o.word(), "applied", "round {round}");
                assert_eq!(m.transfers.len(), 1, "round {round}: the journal kept it");
                assert!(m.records("notes.md"), "round {round}: the entry kept it");
            }
            Err(TransferError::Refused(r)) => {
                assert!(
                    matches!(r.kind, RefusalKind::StalePlan | RefusalKind::Busy),
                    "round {round}: {r:?}"
                );
                assert!(m.transfers.is_empty());
            }
            Err(e) => panic!("round {round}: {e}"),
        }
        match env.unwrap() {
            EnvOutcome::Applied { written } | EnvOutcome::Partial { written, .. } => {
                for path in written {
                    assert!(m.records(&path), "round {round}: {path} kept");
                }
            }
            EnvOutcome::Refused { .. } => {}
        }
    }
}

// Rule 4 as written: the bare plan identity is accepted, and a stale one is
// refused naming what changed where that can be determined.
#[test]
fn the_bare_plan_identity_is_accepted_and_a_stale_one_names_what_it_can() {
    let f = Fixture::new();
    f.write("notes.md", b"one\n");
    f.write("other.md", b"other\n");
    let bare = |path: &str, from, to| plan(&f.ctx(), path, from, to).unwrap().identity;
    let changed = |id: &str| match apply(&f.ctx(), "notes.md", User, Adopted, id, &act(), &clock())
    {
        Err(TransferError::Refused(r)) => {
            assert_eq!(r.kind, RefusalKind::StalePlan);
            r.changed
        }
        other => panic!("{other:?}"),
    };

    // The manifest moved: found among the digests the journal records.
    let id = bare("notes.md", User, Adopted);
    f.transfer("other.md", User, Adopted);
    assert_eq!(changed(&id), ["manifest"]);

    // The file moved to bytes nothing recorded: cannot be determined.
    let id = bare("notes.md", User, Adopted);
    f.write("notes.md", b"two\n");
    assert_eq!(changed(&id), ["undetermined"]);

    // Still current: never refused.
    let id = bare("notes.md", User, Adopted);
    let out = apply(&f.ctx(), "notes.md", User, Adopted, &id, &act(), &clock()).unwrap();
    assert_eq!(out.word(), "applied");
}

// Rule 6: an unrecorded edit is reported ahead of what the inverse move would
// itself refuse.
#[test]
fn a_reversal_reports_an_unrecorded_edit_before_a_refusal_of_the_inverse_move() {
    let f = Fixture::new();
    f.env_apply();
    let record = f.transfer(POINTER, Managed, User).record().clone();
    f.write(POINTER, b"edited by the user\n");
    // Unedited, this reversal is refused as an instruction file; edited, the
    // edit is what is named.
    refused_unchanged(&f, RefusalKind::UnrecordedChange, || f.revert(&record.id));
}
