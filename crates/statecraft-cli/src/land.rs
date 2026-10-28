//! Argument binding for spec 063's read-only wait operation.
use crate::{
    commands::Verb,
    exit::Exit,
    render::{Answer, Format},
};
use statecraft_land::{Event, Gh, Request, SystemClock, duration};
use std::time::Duration;
const USAGE: &str =
    "land wait <pr>... --until <event> --timeout <90s|30m|2h> [--repo <owner/name>] [--json]";
struct Arguments<'a> {
    prs: Vec<u64>,
    event: Event,
    timeout: Duration,
    repo: Option<&'a str>,
}
fn arguments(args: &[String]) -> Result<Arguments<'_>, &'static str> {
    let (mut prs, mut event, mut timeout, mut repo) = (Vec::new(), None, None, None);
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--until" if event.is_none() => event = Some(words.next().ok_or(USAGE)?.parse()?),
            "--timeout" if timeout.is_none() => {
                timeout = Some(duration(words.next().ok_or(USAGE)?)?)
            }
            "--repo" if repo.is_none() => repo = Some(words.next().ok_or(USAGE)?.as_str()),
            value if !value.starts_with('-') => {
                prs.push(value.parse().ok().filter(|p| *p > 0).ok_or(USAGE)?)
            }
            _ => return Err(USAGE),
        }
    }
    if prs.is_empty() {
        return Err(USAGE);
    }
    Ok(Arguments {
        prs,
        event: event.ok_or(USAGE)?,
        timeout: timeout.ok_or(USAGE)?,
        repo,
    })
}
/// Parse, invoke one operation, render the same answer in either format.
pub fn run(args: &[String], json: bool) -> i32 {
    let result = arguments(args).and_then(|a| {
        let directory = std::env::current_dir().map_err(|_| "cannot resolve current directory")?;
        Ok(statecraft_land::wait(
            &Gh::default(),
            &mut SystemClock::default(),
            Request {
                pull_requests: &a.prs,
                event: a.event,
                timeout: a.timeout,
                repository: a.repo,
                directory: &directory,
            },
        ))
    });
    let (value, code) = match result {
        Ok(answer) => {
            let code = answer.exit_code;
            (
                serde_json::to_value(answer).expect("serializable answer"),
                code,
            )
        }
        Err(reason) => (
            serde_json::json!({"event":null,"outcome":"usage","pullRequest":null,"observations":[],"policy":{},"reason":reason}),
            3,
        ),
    };
    let mut summary = String::new();
    if let Some(fields) = value.as_object() {
        for (key, value) in fields {
            if !value.is_null() {
                summary.push_str(&format!(
                    "{key}: {}\n",
                    value
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| value.to_string())
                ));
            }
        }
    }
    let exit = match code {
        0 => Exit::Ok,
        1 => Exit::Finding,
        2 => Exit::Refused,
        3 => Exit::Usage,
        _ => Exit::Failed,
    };
    let format = if json {
        Format::Json(Verb::LandWait)
    } else {
        Format::Human
    };
    print!("{}", Answer::new(value, exit, summary).render(format));
    code
}

/// Bind the explicit landing acts before any other environment or project read.
pub fn act(verb: Verb, args: &[String], json: bool) -> i32 {
    let attempt = statecraft_land::Attempt {
        run_id: std::env::var_os("STATECRAFT_RUN_ID").is_some(),
        attempt: std::env::var_os("STATECRAFT_ATTEMPT").is_some(),
    };
    let directory = std::path::Path::new(".");
    let (value, code) = match verb {
        Verb::LandArm => {
            let answer = statecraft_land::arm::run(directory, args, attempt);
            let code = answer.exit_code;
            (
                serde_json::to_value(answer).expect("serializable answer"),
                code,
            )
        }
        Verb::LandPush => {
            let answer = statecraft_land::push::run(directory, args, attempt);
            let code = answer.exit_code;
            (
                serde_json::to_value(answer).expect("serializable answer"),
                code,
            )
        }
        Verb::LandRestack => {
            let answer = statecraft_land::restack::run(directory, args, attempt);
            let code = answer.exit_code;
            (
                serde_json::to_value(answer).expect("serializable answer"),
                code,
            )
        }
        Verb::LandCancelSuperseded => {
            let answer = statecraft_land::cancel::run(directory, args, attempt);
            let code = answer.exit_code;
            (
                serde_json::to_value(answer).expect("serializable answer"),
                code,
            )
        }
        _ => unreachable!("landing act"),
    };
    let summary = value
        .as_object()
        .unwrap()
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| {
            format!(
                "{k}: {}\n",
                v.as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| v.to_string())
            )
        })
        .collect::<String>();
    let exit = match code {
        0 => Exit::Ok,
        1 => Exit::Finding,
        2 => Exit::Refused,
        3 => Exit::Usage,
        _ => Exit::Failed,
    };
    let format = if json {
        Format::Json(verb)
    } else {
        Format::Human
    };
    print!("{}", Answer::new(value, exit, summary).render(format));
    code
}
