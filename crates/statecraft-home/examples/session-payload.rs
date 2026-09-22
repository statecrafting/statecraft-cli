//! Write the managed-session settings payload, and its digest.
//!
//! Spec 002 section 3.27: the deny floor is delivered per managed session
//! through a supported settings mechanism, and it is **not** written into a
//! user's global settings, where it would apply in every repository that user
//! opens. This prints the exact bytes the `--settings` argument receives, so a
//! session can be started with them and an observation can name which bytes it
//! was made against.
//!
//! ```sh
//! cargo run -q -p statecraft-home --example session-payload > floor.json
//! cargo run -q -p statecraft-home --example session-payload -- --digest
//! ```
//!
//! The digest is over those exact bytes and is what
//! `statecraft_home::startup::admit` compares a claimed observation against: an
//! observation of a session started with other settings is an observation of
//! something else, and is refused rather than admitted.

fn main() {
    let payload = statecraft_home::session::payload_json();
    if std::env::args().any(|a| a == "--digest") {
        println!("{}", statecraft_home::startup::payload_identity());
        return;
    }
    print!("{payload}");
}
