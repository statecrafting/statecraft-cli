//! Thin binding for spec 062's read-only library operation.
use crate::commands::Verb;
use crate::exit::Exit;
use crate::render::{Answer, Format};
use statecraft_home::review_status::{self, Error, Request};
use statecraft_home::setup::GhHost;

fn arguments(args: &[String]) -> Result<(u64, Option<&str>, Option<&str>), Error> {
    let usage = || Error::Usage("review status <pr> [--head <sha>] [--repo <owner/name>] [--json]");
    let pr = args
        .first()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(usage)?;
    let (mut head, mut repo) = (None, None);
    let mut words = args[1..].iter();
    while let Some(word) = words.next() {
        let slot = match word.as_str() {
            "--head" if head.is_none() => &mut head,
            "--repo" if repo.is_none() => &mut repo,
            _ => return Err(usage()),
        };
        *slot = Some(
            words
                .next()
                .filter(|s| !s.starts_with('-'))
                .ok_or_else(usage)?
                .as_str(),
        );
    }
    Ok((pr, head, repo))
}

/// Parse, call one operation, and render its answer without a second rule.
pub fn run(args: &[String], json: bool) -> i32 {
    let result = arguments(args).and_then(|(pr, head, repository)| {
        let directory = std::env::current_dir()
            .map_err(|_| Error::Usage("cannot resolve current directory"))?;
        review_status::status(
            &GhHost::default(),
            Request {
                pull_request: pr,
                head,
                repository,
                directory: &directory,
            },
        )
    });
    let (value, exit) = match result {
        Ok(status) => {
            let exit = match status.outcome.exit_code() {
                0 => Exit::Ok,
                1 => Exit::Finding,
                _ => Exit::Refused,
            };
            (
                serde_json::to_value(status).expect("status is serializable"),
                exit,
            )
        }
        Err(error) => {
            let (outcome, reason, exit) = match error {
                Error::Usage(reason) => ("usage", reason.to_owned(), Exit::Usage),
                Error::Read(error) => ("failed", error.reason(), Exit::Failed),
            };
            (serde_json::json!({"outcome":outcome,"reason":reason}), exit)
        }
    };
    let mut summary = String::new();
    if let Some(fields) = value.as_object() {
        for (key, value) in fields {
            if !value.is_null() {
                if let Some(text) = value.as_str() {
                    summary.push_str(&format!("{key}: {text}\n"));
                } else {
                    summary.push_str(&format!("{key}: {value}\n"));
                }
            }
        }
    }
    let answer = Answer::new(value, exit, summary);
    let format = if json {
        Format::Json(Verb::ReviewStatus)
    } else {
        Format::Human
    };
    print!("{}", answer.render(format));
    answer.exit.code()
}
