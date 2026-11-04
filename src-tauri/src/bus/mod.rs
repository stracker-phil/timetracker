//! Every app-level action is a command with an id (ADR 0038). The native menu, the tray, the page, the
//! palette, links and `timetracker run` run commands by id, so callers never import the features; each
//! feature registers its own (`register` here, `timers::commands::register`, `tracker::commands::register`).
//! `page:<id>` runs one of the page's own commands in the focused window.
//!
//! The bus is generic over its host (the app, or a test double), which reports failures.

pub mod app;
mod command;
mod commands;

pub use app::{Commands, spawn};
pub use command::{Command, Family, Info, RunFuture};
pub use commands::{PAGE_PREFIX, register};

use crate::error::{AppError, AppResult};

use self::command::Run;

/// What the bus needs from the app around it.
pub trait Host: Clone + Send + Sync + 'static {
    fn report_failure(&self, id: &str, error: &AppError);
}

/// A command or family in registration order.
pub enum Listed<'a, H> {
    Command { id: &'a str, info: &'a Info<H> },
    Family(&'a Family<H>),
}

impl<'a, H> Listed<'a, H> {
    /// A family's prefix.
    pub fn id(&self) -> &'a str {
        match self {
            Self::Command { id, .. } => id,
            Self::Family(family) => family.prefix,
        }
    }

    pub fn info(&self) -> &'a Info<H> {
        match self {
            Self::Command { info, .. } => info,
            Self::Family(family) => &family.info,
        }
    }
}

enum Entry<H> {
    Command(Command<H>),
    Family(Family<H>),
}

struct Found<'a, H> {
    info: &'a Info<H>,
    run: &'a Run<H>,
    /// Where the argument starts in the id.
    argument: usize,
}

pub struct CommandBus<H> {
    entries: Vec<Entry<H>>,
}

impl<H> Default for CommandBus<H> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<H: Host> CommandBus<H> {
    /// A later registration of the same id replaces the earlier one.
    pub fn add(&mut self, command: Command<H>) {
        self.entries
            .retain(|entry| !matches!(entry, Entry::Command(c) if c.id == command.id));
        self.entries.push(Entry::Command(command));
    }

    pub fn add_family(&mut self, family: Family<H>) {
        self.entries
            .retain(|entry| !matches!(entry, Entry::Family(f) if f.prefix == family.prefix));
        self.entries.push(Entry::Family(family));
    }

    /// An exact id first, else the family with the longest matching prefix.
    fn find(&self, id: &str) -> Option<Found<'_, H>> {
        let exact = self.entries.iter().find_map(|entry| match entry {
            Entry::Command(c) if c.id == id => Some(Found {
                info: &c.info,
                run: &c.run,
                argument: id.len(),
            }),
            _ => None,
        });
        exact.or_else(|| {
            self.entries
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Family(f) if id.starts_with(f.prefix) => Some(f),
                    _ => None,
                })
                .max_by_key(|f| f.prefix.len())
                .map(|f| Found {
                    info: &f.info,
                    run: &f.run,
                    argument: f.prefix.len(),
                })
        })
    }

    pub fn available<'a>(&'a self, host: &'a H) -> Available<'a, H> {
        Available { bus: self, host }
    }

    pub fn knows(&self, id: &str) -> bool {
        self.find(id).is_some()
    }

    pub fn info(&self, id: &str) -> Option<&Info<H>> {
        self.find(id).map(|found| found.info)
    }

    /// Known and not switched off by its enabled check.
    pub fn is_enabled(&self, host: &H, id: &str) -> bool {
        self.find(id).is_some_and(|found| {
            ask(
                found.info.enabled.as_ref(),
                host,
                &id[found.argument..],
                true,
            )
        })
    }

    /// Known, with a checked check that holds.
    pub fn is_checked(&self, host: &H, id: &str) -> bool {
        self.find(id).is_some_and(|found| {
            ask(
                found.info.checked.as_ref(),
                host,
                &id[found.argument..],
                false,
            )
        })
    }

    pub fn family(&self, prefix: &str) -> Option<&Family<H>> {
        self.entries.iter().find_map(|entry| match entry {
            Entry::Family(f) if f.prefix == prefix => Some(f),
            _ => None,
        })
    }

    pub fn list(&self) -> impl Iterator<Item = Listed<'_, H>> {
        self.entries.iter().map(|entry| match entry {
            Entry::Command(c) => Listed::Command {
                id: c.id,
                info: &c.info,
            },
            Entry::Family(f) => Listed::Family(f),
        })
    }

    /// The command's work, ready to await; refused when it is unknown or disabled.
    pub fn start(&self, host: H, id: &str) -> AppResult<RunFuture> {
        let found = self
            .find(id)
            .ok_or_else(|| AppError::NotFound(format!("Unknown command “{id}”.")))?;
        let argument = &id[found.argument..];
        if !ask(found.info.enabled.as_ref(), &host, argument, true) {
            return Err(AppError::InvalidInput(format!(
                "“{id}” isn't available right now."
            )));
        }
        Ok((found.run)(host, argument.to_string()))
    }

    /// Returns the failure instead of reporting it, for callers that report it themselves.
    pub async fn run_or_err(&self, host: H, id: &str) -> AppResult<()> {
        self.start(host, id)?.await
    }

    /// Logs an unknown id; a failure is logged and reported by the host.
    pub async fn run(&self, host: H, id: &str) {
        if !self.knows(id) {
            log::warn!("Unknown command: {id}");
            return;
        }
        if let Err(e) = self.run_or_err(host.clone(), id).await {
            log::error!("{id} failed: {e}");
            host.report_failure(id, &e);
        }
    }
}

/// Which commands exist and can run now, and which are checked; menus leave out unknown ones and grey
/// disabled ones.
pub trait CommandStatus {
    fn knows(&self, id: &str) -> bool;
    fn is_enabled(&self, id: &str) -> bool;
    fn is_checked(&self, id: &str) -> bool;
}

/// The bus seen from one host.
pub struct Available<'a, H> {
    bus: &'a CommandBus<H>,
    host: &'a H,
}

impl<H: Host> CommandStatus for Available<'_, H> {
    fn knows(&self, id: &str) -> bool {
        self.bus.knows(id)
    }

    fn is_enabled(&self, id: &str) -> bool {
        self.bus.is_enabled(self.host, id)
    }

    fn is_checked(&self, id: &str) -> bool {
        self.bus.is_checked(self.host, id)
    }
}

/// `missing` when the command has no such check.
fn ask<H>(check: Option<&command::Check<H>>, host: &H, argument: &str, missing: bool) -> bool {
    check.map_or(missing, |check| check(host, argument))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A host that notes what ran and what failed, with a switch the commands' checks read.
    #[derive(Clone, Default)]
    struct TestHost {
        ran: Arc<Mutex<Vec<String>>>,
        failures: Arc<Mutex<Vec<(String, AppError)>>>,
        on: Arc<AtomicBool>,
    }

    impl TestHost {
        fn note(&self, entry: String) {
            self.ran.lock().unwrap().push(entry);
        }

        fn ran(&self) -> Vec<String> {
            self.ran.lock().unwrap().clone()
        }

        fn failures(&self) -> Vec<(String, AppError)> {
            self.failures.lock().unwrap().clone()
        }

        fn switch(&self, on: bool) {
            self.on.store(on, Ordering::SeqCst);
        }

        fn is_on(&self) -> bool {
            self.on.load(Ordering::SeqCst)
        }
    }

    impl Host for TestHost {
        fn report_failure(&self, id: &str, error: &AppError) {
            self.failures
                .lock()
                .unwrap()
                .push((id.to_string(), error.clone()));
        }
    }

    fn noting(id: &'static str) -> Command<TestHost> {
        Command::<TestHost>::new(id, "Title").run(move |host| async move {
            host.note(id.to_string());
            Ok(())
        })
    }

    fn failing(id: &'static str, error: AppError) -> Command<TestHost> {
        Command::<TestHost>::new(id, "Title").run(move |_| {
            let error = error.clone();
            async move { Err(error) }
        })
    }

    fn noting_family(prefix: &'static str) -> Family<TestHost> {
        Command::<TestHost>::new(prefix, "Title")
            .family("<argument>")
            .run(move |host, argument| async move {
                host.note(format!("{prefix}[{argument}]"));
                Ok(())
            })
    }

    fn bus_of(commands: Vec<Command<TestHost>>) -> CommandBus<TestHost> {
        let mut bus = CommandBus::default();
        for command in commands {
            bus.add(command);
        }
        bus
    }

    fn bus_of_families(families: Vec<Family<TestHost>>) -> CommandBus<TestHost> {
        let mut bus = CommandBus::default();
        for family in families {
            bus.add_family(family);
        }
        bus
    }

    #[tokio::test]
    async fn an_unknown_id_is_not_found() {
        let bus = bus_of(vec![noting("a:one")]);
        let host = TestHost::default();

        let result = bus.run_or_err(host.clone(), "a:two").await;

        assert_eq!(
            result,
            Err(AppError::NotFound("Unknown command “a:two”.".into()))
        );
        assert_eq!(host.ran(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn a_known_command_runs() {
        let bus = bus_of(vec![noting("a:one"), noting("a:two")]);
        let host = TestHost::default();

        assert_eq!(bus.run_or_err(host.clone(), "a:two").await, Ok(()));

        assert_eq!(host.ran(), ["a:two"]);
    }

    #[tokio::test]
    async fn run_or_err_returns_the_failure_without_reporting_it() {
        let error = AppError::Io("disk".into());
        let bus = bus_of(vec![failing("a:fail", error.clone())]);
        let host = TestHost::default();

        let result = bus.run_or_err(host.clone(), "a:fail").await;

        assert_eq!(result, Err(error));
        assert_eq!(host.failures(), vec![]);
    }

    #[tokio::test]
    async fn run_hands_a_failure_to_the_host() {
        let error = AppError::Io("disk".into());
        let bus = bus_of(vec![failing("a:fail", error.clone())]);
        let host = TestHost::default();

        bus.run(host.clone(), "a:fail").await;

        assert_eq!(host.failures(), vec![("a:fail".to_string(), error)]);
    }

    #[tokio::test]
    async fn run_stays_quiet_for_an_unknown_id_and_for_success() {
        let bus = bus_of(vec![noting("a:one")]);
        let host = TestHost::default();

        bus.run(host.clone(), "a:missing").await;
        bus.run(host.clone(), "a:one").await;

        assert_eq!(host.failures(), vec![]);
        assert_eq!(host.ran(), ["a:one"]);
    }

    #[tokio::test]
    async fn run_hands_a_refused_disabled_command_to_the_host() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::<TestHost>::new("a:gated", "Gated")
                .enabled_when(TestHost::is_on)
                .run_now(|host| host.note("gated".into())),
        );
        let host = TestHost::default();

        bus.run(host.clone(), "a:gated").await;

        assert_eq!(host.ran(), Vec::<String>::new());
        assert_eq!(
            host.failures(),
            vec![(
                "a:gated".to_string(),
                AppError::InvalidInput("“a:gated” isn't available right now.".into())
            )]
        );
    }

    #[tokio::test]
    async fn a_family_gets_the_argument_after_its_prefix() {
        let bus = bus_of_families(vec![noting_family("tracker:pause:")]);
        let host = TestHost::default();

        for id in [
            "tracker:pause:60",
            "tracker:pause:tomorrow",
            "tracker:pause:",
        ] {
            bus.run_or_err(host.clone(), id).await.unwrap();
        }

        assert_eq!(
            host.ran(),
            [
                "tracker:pause:[60]",
                "tracker:pause:[tomorrow]",
                "tracker:pause:[]"
            ]
        );
    }

    #[tokio::test]
    async fn the_longest_matching_prefix_wins() {
        let bus = bus_of_families(vec![noting_family("app:"), noting_family("app:settings:")]);
        let host = TestHost::default();

        bus.run_or_err(host.clone(), "app:settings:capture")
            .await
            .unwrap();
        bus.run_or_err(host.clone(), "app:other").await.unwrap();

        assert_eq!(host.ran(), ["app:settings:[capture]", "app:[other]"]);
    }

    #[tokio::test]
    async fn an_exact_id_beats_a_family_that_matches_it() {
        let mut bus = bus_of(vec![noting("app:settings:special")]);
        bus.add_family(noting_family("app:settings:"));
        let host = TestHost::default();

        bus.run_or_err(host.clone(), "app:settings:special")
            .await
            .unwrap();
        bus.run_or_err(host.clone(), "app:settings:capture")
            .await
            .unwrap();

        assert_eq!(
            host.ran(),
            ["app:settings:special", "app:settings:[capture]"]
        );
    }

    #[tokio::test]
    async fn a_disabled_command_is_refused_and_does_not_run() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::<TestHost>::new("a:gated", "Gated")
                .enabled_when(TestHost::is_on)
                .run_now(|host| host.note("gated".into())),
        );
        let host = TestHost::default();

        let refused = bus.run_or_err(host.clone(), "a:gated").await;

        assert_eq!(
            refused,
            Err(AppError::InvalidInput(
                "“a:gated” isn't available right now.".into()
            ))
        );
        assert_eq!(host.ran(), Vec::<String>::new());

        host.switch(true);
        assert_eq!(bus.run_or_err(host.clone(), "a:gated").await, Ok(()));
        assert_eq!(host.ran(), ["gated"]);
    }

    #[tokio::test]
    async fn a_family_is_enabled_per_argument() {
        let mut bus = CommandBus::default();
        bus.add_family(
            Command::<TestHost>::new("pause:", "Pause")
                .family("<length>")
                .enabled_for(|_, length| length == "60")
                .run(|host, length| async move {
                    host.note(length);
                    Ok(())
                }),
        );
        let host = TestHost::default();

        assert!(bus.is_enabled(&host, "pause:60"));
        assert!(!bus.is_enabled(&host, "pause:15"));
        assert_eq!(bus.run_or_err(host.clone(), "pause:60").await, Ok(()));
        assert_eq!(
            bus.run_or_err(host.clone(), "pause:15").await,
            Err(AppError::InvalidInput(
                "“pause:15” isn't available right now.".into()
            ))
        );
        assert_eq!(host.ran(), ["60"]);
    }

    #[test]
    fn a_family_is_checked_per_argument() {
        let mut bus = CommandBus::default();
        bus.add_family(
            Command::<TestHost>::new("view:", "View")
                .family("<name>")
                .checked_for(|_, name| name == "week")
                .run(|_, _| async { Ok(()) }),
        );
        let host = TestHost::default();

        assert!(bus.is_checked(&host, "view:week"));
        assert!(!bus.is_checked(&host, "view:day"));
    }

    #[test]
    fn a_command_without_a_check_is_enabled_and_unchecked() {
        let bus = bus_of(vec![noting("a:one")]);
        let host = TestHost::default();

        assert!(bus.is_enabled(&host, "a:one"));
        assert!(!bus.is_checked(&host, "a:one"));
    }

    #[test]
    fn an_unknown_id_is_neither_enabled_nor_checked() {
        let bus = bus_of(vec![noting("a:one")]);
        let host = TestHost::default();

        assert!(!bus.knows("a:two"));
        assert!(!bus.is_enabled(&host, "a:two"));
        assert!(!bus.is_checked(&host, "a:two"));
    }

    #[test]
    fn a_checked_command_follows_its_check() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::<TestHost>::new("a:toggle", "Toggle")
                .checked_when(TestHost::is_on)
                .run_now(|_| {}),
        );
        let host = TestHost::default();

        assert!(!bus.is_checked(&host, "a:toggle"));
        host.switch(true);
        assert!(bus.is_checked(&host, "a:toggle"));
    }

    #[test]
    fn the_available_view_reports_the_hosts_state() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::<TestHost>::new("a:gated", "Gated")
                .enabled_when(TestHost::is_on)
                .checked_when(TestHost::is_on)
                .run_now(|_| {}),
        );
        let host = TestHost::default();

        let off = bus.available(&host);
        assert!(off.knows("a:gated"));
        assert!(!off.is_enabled("a:gated"));
        assert!(!off.is_checked("a:gated"));
        assert!(!off.knows("a:other"));

        host.switch(true);
        let on = bus.available(&host);
        assert!(on.is_enabled("a:gated"));
        assert!(on.is_checked("a:gated"));
    }

    #[tokio::test]
    async fn registering_an_id_again_replaces_the_command() {
        let mut bus = bus_of(vec![noting("a:one")]);
        bus.add(
            Command::<TestHost>::new("a:one", "Replacement").run(|host| async move {
                host.note("replacement".into());
                Ok(())
            }),
        );
        let host = TestHost::default();

        bus.run_or_err(host.clone(), "a:one").await.unwrap();

        assert_eq!(host.ran(), ["replacement"]);
        assert_eq!(bus.list().count(), 1);
        assert_eq!(bus.info("a:one").unwrap().title, "Replacement");
    }

    #[tokio::test]
    async fn registering_a_prefix_again_replaces_the_family() {
        let mut bus = bus_of_families(vec![noting_family("pause:")]);
        bus.add_family(
            Command::<TestHost>::new("pause:", "Replacement")
                .family("<length>")
                .run(|host, length| async move {
                    host.note(format!("replacement {length}"));
                    Ok(())
                }),
        );
        let host = TestHost::default();

        bus.run_or_err(host.clone(), "pause:60").await.unwrap();

        assert_eq!(host.ran(), ["replacement 60"]);
        assert_eq!(bus.list().count(), 1);
    }

    #[test]
    fn the_list_keeps_registration_order_and_names_a_family_by_its_prefix() {
        let mut bus = bus_of(vec![noting("a:first")]);
        bus.add_family(noting_family("b:family:"));
        bus.add(noting("c:last"));

        let ids: Vec<&str> = bus.list().map(|listed| listed.id()).collect();

        assert_eq!(ids, ["a:first", "b:family:", "c:last"]);
    }

    #[test]
    fn a_family_is_found_by_its_prefix_with_its_argument_name() {
        let bus = bus_of_families(vec![noting_family("pause:")]);

        assert_eq!(bus.family("pause:").unwrap().arg, "<argument>");
        assert!(bus.family("pause").is_none());
        assert!(bus.family("pause:60").is_none());
    }

    #[test]
    fn a_command_is_visible_and_does_not_quit_with_its_title_as_help_by_default() {
        let bus = bus_of(vec![noting("a:one")]);

        let info = bus.info("a:one").unwrap();

        assert_eq!(info.title, "Title");
        assert_eq!(info.help, "Title");
        assert!(!info.hidden);
        assert!(!info.quits);
    }

    #[test]
    fn a_command_carries_its_help_and_flags() {
        let mut bus = CommandBus::default();
        bus.add(
            Command::<TestHost>::new("a:quit", "Quit")
                .help("Leave")
                .hidden()
                .quits()
                .run_now(|_| {}),
        );

        let info = bus.info("a:quit").unwrap();

        assert_eq!(info.title, "Quit");
        assert_eq!(info.help, "Leave");
        assert!(info.hidden);
        assert!(info.quits);
    }

    #[test]
    fn info_of_an_unknown_id_is_none() {
        let bus = bus_of(vec![noting("a:one")]);

        assert!(bus.info("a:two").is_none());
    }
}
