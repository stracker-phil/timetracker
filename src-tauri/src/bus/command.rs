//! Building a command: `Command::new(id, title)`, its flags and checks, then its work.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::Host;
use crate::error::AppResult;

pub type RunFuture = Pin<Box<dyn Future<Output = AppResult<()>> + Send>>;
pub(super) type Run<H> = Arc<dyn Fn(H, String) -> RunFuture + Send + Sync>;
/// Asked with the family's argument; a plain command gets "".
pub(super) type Check<H> = Arc<dyn Fn(&H, &str) -> bool + Send + Sync>;

/// What help, menus and the palette know about a command.
pub struct Info<H> {
    pub title: &'static str,
    /// One line for `timetracker commands`.
    pub help: &'static str,
    /// Left out of the command list (closing whichever window has focus, Settings at a page).
    pub hidden: bool,
    /// The app is gone once it ran, so a caller answers first.
    pub quits: bool,
    pub(super) enabled: Option<Check<H>>,
    pub(super) checked: Option<Check<H>>,
}

impl<H> Info<H> {
    fn new(title: &'static str) -> Self {
        Self {
            title,
            help: title,
            hidden: false,
            quits: false,
            enabled: None,
            checked: None,
        }
    }
}

/// A command before it gets its `run`.
pub struct Spec<H> {
    pub(super) id: &'static str,
    pub(super) info: Info<H>,
}

impl<H: Host> Spec<H> {
    pub fn help(mut self, help: &'static str) -> Self {
        self.info.help = help;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.info.hidden = true;
        self
    }

    pub fn quits(mut self) -> Self {
        self.info.quits = true;
        self
    }

    /// A disabled command doesn't run, is greyed in menus and left out of the palette.
    pub fn enabled_when(mut self, check: impl Fn(&H) -> bool + Send + Sync + 'static) -> Self {
        self.info.enabled = Some(Arc::new(move |host, _| check(host)));
        self
    }

    /// The checkmark of its menu item.
    pub fn checked_when(mut self, check: impl Fn(&H) -> bool + Send + Sync + 'static) -> Self {
        self.info.checked = Some(Arc::new(move |host, _| check(host)));
        self
    }

    pub fn run<F, Fut>(self, run: F) -> Command<H>
    where
        F: Fn(H) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AppResult<()>> + Send + 'static,
    {
        Command {
            id: self.id,
            info: self.info,
            run: Arc::new(move |host, _| Box::pin(run(host))),
        }
    }

    /// For work that returns at once.
    pub fn run_now(self, run: impl Fn(&H) + Send + Sync + 'static) -> Command<H> {
        let run = Arc::new(run);
        self.run(move |host| {
            let run = Arc::clone(&run);
            async move {
                run(&host);
                Ok(())
            }
        })
    }

    /// A family: the id is the prefix, prefix + argument runs it.
    pub fn family(self, arg: &'static str) -> FamilySpec<H> {
        FamilySpec {
            prefix: self.id,
            arg,
            info: self.info,
        }
    }
}

pub struct Command<H> {
    pub(super) id: &'static str,
    pub(super) info: Info<H>,
    pub(super) run: Run<H>,
}

impl<H: Host> Command<H> {
    /// A command is built in two steps: its spec, then `run` adds the work.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(id: &'static str, title: &'static str) -> Spec<H> {
        Spec {
            id,
            info: Info::new(title),
        }
    }
}

pub struct FamilySpec<H> {
    prefix: &'static str,
    arg: &'static str,
    info: Info<H>,
}

impl<H: Host> FamilySpec<H> {
    /// Like `Spec::enabled_when`, asked with the argument.
    pub fn enabled_for(mut self, check: impl Fn(&H, &str) -> bool + Send + Sync + 'static) -> Self {
        self.info.enabled = Some(Arc::new(check));
        self
    }

    /// Like `Spec::checked_when`, asked with the argument.
    pub fn checked_for(mut self, check: impl Fn(&H, &str) -> bool + Send + Sync + 'static) -> Self {
        self.info.checked = Some(Arc::new(check));
        self
    }

    pub fn run<F, Fut>(self, run: F) -> Family<H>
    where
        F: Fn(H, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = AppResult<()>> + Send + 'static,
    {
        Family {
            prefix: self.prefix,
            arg: self.arg,
            info: self.info,
            run: Arc::new(move |host, argument| Box::pin(run(host, argument))),
        }
    }
}

pub struct Family<H> {
    pub prefix: &'static str,
    /// The argument as the command list shows it, e.g. `<page>`.
    pub arg: &'static str,
    pub(super) info: Info<H>,
    pub(super) run: Run<H>,
}
