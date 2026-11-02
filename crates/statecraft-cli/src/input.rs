//! The supervisor's operator-input desk during a live attempt (spec 053).
//!
//! [`Steered`] wraps the attempt's own watch: after each event the inner
//! watch has judged, it runs one pass of the run record's
//! [`Desk`](statecraft_run::input::Desk) over the chain this supervisor holds
//! under the repository lock, placing notes in the attempt's exchange
//! directory and recording each state. [`Steered::close`] runs the last pass
//! once the process has ended.
//!
//! The chain is one of the files spec 003 section 3.5.1 digests before the
//! spawn and again after it. What the desk appends while the child runs is
//! therefore reported, as the launch watch reports its own records, by the
//! exact bytes the file must hold now: the bytes read before the spawn and
//! every line this supervisor appended since. Anything else in the file is
//! still a change the snapshot reports.
//!
//! The inbox is [`NoInbox`](statecraft_run::input::NoInbox) until the owner
//! decides how another process reaches a live attempt's supervisor (spec 053
//! section 5, 2026-11-01), so in a run today the desk finds nothing to do.

use statecraft_adapter::supervisor::{Control, Watch};
use statecraft_adapter_claude_code::stream::ProviderEvent;
use statecraft_environment::time::{Clock, rfc3339_utc};
use statecraft_run::input::{Desk, Inbox, Tick};
use statecraft_run::record::Chain;
use std::path::{Path, PathBuf};

/// The events a managed attempt's watch judges: the line and its decoding.
pub type Event = (usize, ProviderEvent);

/// One attempt's watch, with the desk run after every event.
pub struct Steered<'a> {
    inner: &'a mut dyn Watch<Event>,
    chain: &'a mut Chain,
    inbox: &'a mut dyn Inbox,
    clock: &'a dyn Clock,
    desk: Desk,
    placement: statecraft_home::input::ExchangePlacement,
    path: PathBuf,
    bytes: Vec<u8>,
    appended: bool,
    failures: Vec<String>,
}

/// What the desk left behind when the attempt ended.
#[derive(Debug, Default)]
pub struct Steering {
    /// The chain file and the bytes it must now hold, where the desk
    /// appended anything while the child ran.
    pub wrote: Option<(PathBuf, Vec<u8>)>,
    /// Every submission refused and every placement not made.
    pub failures: Vec<String>,
}

impl<'a> Steered<'a> {
    /// Wrap `inner` for attempt `attempt` of `run_id`. Called after the
    /// record snapshot and before the spawn, so the chain bytes read here are
    /// the ones the snapshot digested.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        inner: &'a mut dyn Watch<Event>,
        chain: &'a mut Chain,
        inbox: &'a mut dyn Inbox,
        clock: &'a dyn Clock,
        home: &Path,
        root: &Path,
        exchange: &Path,
        attempt: (&str, u32),
    ) -> Self {
        let path = statecraft_run::record::chain_path(home, root);
        let bytes = std::fs::read(&path).unwrap_or_default();
        Self {
            inner,
            chain,
            inbox,
            clock,
            desk: Desk::new(attempt.0, attempt.1),
            placement: statecraft_home::input::ExchangePlacement::new(exchange),
            path,
            bytes,
            appended: false,
            failures: Vec::new(),
        }
    }

    /// Fold in what one pass appended and reported.
    fn kept(&mut self, before: usize, tick: Tick) {
        for record in &self.chain.records()[before..] {
            // The line `Chain::append` wrote, serialized the same way.
            if let Ok(line) = serde_json::to_vec(record) {
                self.bytes.extend_from_slice(&line);
                self.bytes.push(b'\n');
                self.appended = true;
            }
        }
        self.failures.extend(tick.failures);
    }

    /// The last pass, after the process has ended and before the record is
    /// digested again.
    pub fn close(mut self) -> Steering {
        let before = self.chain.records().len();
        let now = self.clock.now_unix();
        let tick = self
            .desk
            .close(self.chain, &mut self.placement, now, &rfc3339_utc(now));
        self.kept(before, tick);
        Steering {
            wrote: self
                .appended
                .then(|| (self.path.clone(), self.bytes.clone())),
            failures: std::mem::take(&mut self.failures),
        }
    }
}

impl Watch<Event> for Steered<'_> {
    fn spawned(&mut self, pid: u32) -> Result<(), String> {
        self.inner.spawned(pid)
    }

    fn event(&mut self, event: &Event) -> Control {
        let control = self.inner.event(event);
        let before = self.chain.records().len();
        let now = self.clock.now_unix();
        let tick = self.desk.tick(
            self.chain,
            &mut self.placement,
            self.inbox,
            now,
            &rfc3339_utc(now),
        );
        self.kept(before, tick);
        control
    }
}
