//! Commit a harness requirement into a project's manifest, as an explicit act.
//!
//! Spec 002 section 3.25's "explicit upgrade", with a way to invoke it. The
//! library has the whole contract; the command tree does not carry a verb for
//! it, because the verb table is spec `006`'s requirement and adding an entry
//! to it is `006`'s change to make rather than this spec's. Until that change
//! is proposed, this is how the act is performed: an example rather than a
//! product surface, deliberately, so nothing reads it as one.
//!
//! It is what the live-session acceptance uses to build a **valid,
//! product-generated** fixture manifest. A hand-written `{}` at the manifest
//! path satisfies the project gate every shipped hook tests and satisfies
//! nothing else: it carries no pins, no requirement and no schema version, so
//! a session started against it could never reach section 3.25's `exact`
//! standing and an acceptance run on it would be measuring the gate rather
//! than the contract.
//!
//! ```sh
//! cargo run -p statecraft-home --example require-harness -- <project> <home>
//! ```
//!
//! Both arguments are directories. The project gets `.statecraft/`, a manifest
//! and the managed instructions if it has none; the home gets the shipped
//! harness revision installed if it is absent. Re-running it is safe: the
//! install is content addressed and idempotent, and a requirement already
//! equal to the shipped revision is reported as unchanged rather than
//! rewritten.

use statecraft_environment::manifest::{Manifest, Pins};
use statecraft_home::{harness, home::Layout, project, required};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(project_dir), Some(home_dir)) = (args.next(), args.next()) else {
        eprintln!(
            "usage: require-harness <project-directory> <product-home-directory>\n\
             \n\
             Commits the shipped harness revision as the project's required identity\n\
             (spec 002 section 3.25), installing that revision under the home first."
        );
        std::process::exit(3);
    };
    let root = std::path::PathBuf::from(&project_dir);
    let layout = Layout::new(&home_dir);

    // 1. The revision, installed under the home. Content addressed, so this is
    //    idempotent and a second run writes nothing.
    let installed = harness::install(&layout, &harness::shipped())?;
    println!(
        "harness   {} ({} file(s), {} written, {} unchanged)",
        installed.revision.id,
        installed.revision.files.len(),
        installed.written.len(),
        installed.unchanged.len()
    );
    println!("digest    {}", installed.revision.digest);

    // 2. The project area, if it has none. The manifest is written through the
    //    governed contract, with real pins, not assembled as JSON here.
    std::fs::create_dir_all(root.join(project::AREA))?;
    let mut manifest = match Manifest::read(&root)? {
        Some(existing) => {
            println!("manifest  read, version {}", existing.version);
            existing
        }
        None => {
            let created = Manifest::new(Pins {
                product: env!("CARGO_PKG_VERSION").to_string(),
                spec_spine: "0.20.0".to_string(),
                adapters: Default::default(),
            });
            println!("manifest  created, version {}", created.version);
            created
        }
    };
    let instructions = statecraft_environment::claimant::resolve(&root, project::INSTRUCTIONS);
    if !instructions.exists() {
        std::fs::write(&instructions, project::managed_instructions())?;
        println!("wrote     {}", project::INSTRUCTIONS);
    }

    // 3. The explicit upgrade: planned first, then applied, then written. The
    //    plan is a read and says what the act would change, including whether
    //    section 3.24's settings consent is asked again.
    let upgrade = required::plan_upgrade(&layout, &manifest, &installed.revision.digest)?;
    println!("upgrade   {}", upgrade.describe());
    required::apply_upgrade(&mut manifest, &upgrade);
    manifest.write(&root)?;

    // 4. And the standing that results, read back from what was written.
    let written = Manifest::read(&root)?.expect("the manifest was just written");
    let standing = required::evaluate(&layout, &written, Some(&installed.revision.digest));
    println!("standing  {}", standing.describe());
    println!("qualified {}", standing.permits_managed_execution());
    if !standing.permits_managed_execution() {
        std::process::exit(1);
    }
    Ok(())
}
