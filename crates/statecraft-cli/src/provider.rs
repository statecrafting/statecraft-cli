//! `provider token set`, `provider token clear` and `provider token show`
//! (spec 042 section 3.2).
//!
//! `set` reads the token from standard input, never from an argument, so it
//! is not in the process table or a shell history. No answer, message or
//! rendering carries the token; `show` says only whether one is stored.

use serde_json::{Value, json};
use statecraft_home::provider_token::{self, TokenError};
use std::io::Read;
use std::path::Path;

use crate::exit::Exit;
use crate::render::{Answer, ErrorKind};

/// What a provider verb returns.
pub type Outcome = Result<Answer<Value>, String>;

fn no_operands(rest: &[String], usage: &str) -> Result<(), String> {
    match rest.iter().find(|a| a.as_str() != "--json") {
        Some(_) => Err(usage.to_string()),
        None => Ok(()),
    }
}

fn refusal(e: &TokenError) -> Answer<Value> {
    let kind = match e {
        TokenError::Io(_) => {
            return Answer::new(json!({}), Exit::Failed, e.to_string()).with_kind(ErrorKind::Io);
        }
        TokenError::Malformed(_) => ErrorKind::Validation,
        _ => ErrorKind::Refused,
    };
    Answer::new(json!({}), Exit::Refused, e.to_string()).with_kind(kind)
}

/// `provider token set`: store the token read from standard input.
pub fn token_set(home: &Path, rest: &[String]) -> Outcome {
    no_operands(
        rest,
        "usage: provider token set   (the token is read from standard input)",
    )?;
    let mut input = Vec::new();
    if std::io::stdin()
        .take(64 * 1024)
        .read_to_end(&mut input)
        .is_err()
    {
        return Ok(
            Answer::new(json!({}), Exit::Failed, "standard input could not be read")
                .with_kind(ErrorKind::Io),
        );
    }
    Ok(match provider_token::set(home, &input) {
        Ok(()) => Answer::new(
            json!({"stored": true}),
            Exit::Ok,
            "the provider token is stored, owner-only, under the product home",
        ),
        Err(e) => refusal(&e),
    })
}

/// `provider token clear`: remove the stored token.
pub fn token_clear(home: &Path, rest: &[String]) -> Outcome {
    no_operands(rest, "usage: provider token clear")?;
    Ok(match provider_token::clear(home) {
        Ok(removed) => Answer::new(
            json!({"removed": removed}),
            Exit::Ok,
            if removed {
                "the provider token is removed"
            } else {
                "no provider token was stored"
            },
        ),
        Err(e) => refusal(&e),
    })
}

/// `provider token show`: whether a usable token is stored. Never the token.
pub fn token_show(home: &Path, rest: &[String]) -> Outcome {
    no_operands(rest, "usage: provider token show")?;
    Ok(match provider_token::read(home) {
        Ok(_) => Answer::new(
            json!({"stored": true, "usable": true}),
            Exit::Ok,
            "a provider token is stored",
        ),
        Err(TokenError::Absent) => Answer::new(
            json!({"stored": false, "usable": false}),
            Exit::Ok,
            format!(
                "no provider token is stored; confined provider launches refuse until `{}`",
                provider_token::SET_COMMAND
            ),
        ),
        Err(e) => refusal(&e),
    })
}
