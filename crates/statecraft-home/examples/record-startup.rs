//! Write a managed session's startup record from evidence that was captured.
//!
//! Spec 002 section 3.26. The record's seven fields are read from the project,
//! the home and the manifest; the three evidence classes are supplied
//! separately, because they are three different measurements and only one of
//! them can be made by reading anything.
//!
//! ```sh
//! cargo run -q -p statecraft-home --example record-startup -- \
//!   <project> <home> <session-id> [<capture-dir>]
//! ```
//!
//! The optional fourth argument is the live-session observation, and it is
//! **admitted rather than believed**. Spec 002 sections 3.29 and 3.30: the
//! directory holds the three launch records `startup capture` wrote, and the
//! admission judges each control from correlated structured events and each
//! invocation against its launch. There is no argument that asserts
//! qualification, and passing one is not possible rather than discouraged.
//!
//! Omit it and the record is written with the observation absent and says so.
//! That is the ordinary case and it is not a failure: a session that was not
//! observed is recorded as one that was not observed.
//!
//! **This is an example of calling the boundary, not the operator's route.**
//! `statecraft-cli startup record`, `startup capture` and `startup qualify` are
//! the verbs, and spec 006 sections 3.11.1 and 3.11.2 are where they are
//! required. This
//! stays runnable as a second caller of the same library operations.

use statecraft_environment::manifest::Manifest;
use statecraft_home::admission;
use statecraft_home::startup::{self, AdapterIdentity, Observation, StartupRecord, Supply};
use statecraft_home::{delivery, home::Layout, required};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 && args.len() != 4 {
        eprintln!(
            "usage: record-startup <project> <home> <session-id> \
             [<capture-dir>]"
        );
        std::process::exit(3);
    }
    let root = std::path::PathBuf::from(&args[0]);
    let layout = Layout::new(&args[1]);
    let session_id = &args[2];

    let manifest = Manifest::read(&root)?.ok_or("the project holds no manifest")?;

    // Evidence class 1: the documented load rule, evaluated against the tree.
    let rule = delivery::load_rules()
        .into_iter()
        .find(|r| r.harness == delivery::native_homes()[0].harness)
        .ok_or("no load rule for this harness")?;
    let verdict = delivery::evaluate(&root, &rule);

    // Section 3.25: what is required, and what answered. The resolved identity
    // is the revision installed under the required identity, which is the only
    // one this product would have used; when that is missing or corrupt the
    // standing says so and the record carries the refusal.
    let required_digest = required::required_of(&manifest).map(str::to_string);
    let standing = required::evaluate(&layout, &manifest, required_digest.as_deref());
    let resolved = standing
        .permits_managed_execution()
        .then(|| required_digest.clone())
        .flatten();

    // Evidence class 2: whether the bytes were read and handed over. This
    // example delivers nothing, and records that rather than implying it.
    let supply = Supply::NotAttempted {
        reason: "this recorder reads and records; it performs no delivery, so nothing \
                 about one is established here"
            .to_string(),
    };

    // Evidence class 3: a live session, admitted or absent.
    let observation = if args.len() == 4 {
        // Reading and judging are two calls, so the two failures stay
        // distinguishable: a capture that is not there is not a capture that
        // showed no refusal.
        let evidence = match admission::load(std::path::Path::new(&args[3])) {
            Ok(evidence) => evidence,
            Err(why) => {
                eprintln!("the captures could not be read: {why}");
                std::process::exit(2);
            }
        };
        match startup::admit(&evidence) {
            Ok(observed) => observed,
            Err(why) => {
                // Not admitted is not "recorded more weakly". Nothing is
                // written, because a record carrying a rejected claim as an
                // absence would lose the fact that a claim was made and
                // refused.
                eprintln!("the claimed observation is not admitted: {why}");
                std::process::exit(1);
            }
        }
    } else {
        Observation::NotObserved {
            reason: "no live session was observed for this start".to_string(),
        }
    };

    let record = startup::assemble(
        &root,
        session_id,
        &statecraft_environment::time::rfc3339_utc(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs() as i64,
        ),
        &manifest,
        AdapterIdentity {
            name: "claude-code".into(),
            harness: "claude-code".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        },
        verdict,
        standing,
        resolved,
        supply,
        observation,
    )?;

    record.write(&root)?;
    println!("{}", record.describe());
    println!(
        "written   {}",
        StartupRecord::path(&root, session_id).display()
    );
    Ok(())
}
