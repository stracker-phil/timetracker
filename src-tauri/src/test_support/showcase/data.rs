//! The tables of the showcase: the apps each kind of work uses, the clients and projects, and the
//! schedules the days rotate through.

use crate::db::away::AwayReason;

/// `(app, bundle id, window title, url)`.
pub(super) type Window = (
    &'static str,
    &'static str,
    &'static str,
    Option<&'static str>,
);

const ZED: (&str, &str) = ("Zed", "dev.zed.Zed");
const ARC: (&str, &str) = ("Arc", "company.thebrowser.Browser");
const TERMINAL: (&str, &str) = ("Terminal", "com.apple.Terminal");
const SLACK: (&str, &str) = ("Slack", "com.tinyspeck.slackmacgap");
const MAIL: (&str, &str) = ("Mail", "com.apple.mail");
const XCODE: (&str, &str) = ("Xcode", "com.apple.dt.Xcode");

const fn window(app: (&'static str, &'static str), title: &'static str) -> Window {
    (app.0, app.1, title, None)
}

const fn page(title: &'static str, url: &'static str) -> Window {
    (ARC.0, ARC.1, title, Some(url))
}

#[derive(Clone, Copy)]
pub(super) enum Work {
    Admin,
    Meeting,
    Web,
    Support,
    Ios,
    Design,
    Api,
    Writing,
}

impl Work {
    pub(super) fn windows(self) -> &'static [Window] {
        match self {
            Work::Admin => ADMIN_WINDOWS,
            Work::Meeting => MEETING_WINDOWS,
            Work::Web => WEB_WINDOWS,
            Work::Support => SUPPORT_WINDOWS,
            Work::Ios => IOS_WINDOWS,
            Work::Design => DESIGN_WINDOWS,
            Work::Api => API_WINDOWS,
            Work::Writing => WRITING_WINDOWS,
        }
    }
}

const ADMIN_WINDOWS: &[Window] = &[
    window(MAIL, "Inbox"),
    (
        "Fantastical",
        "com.flexibits.fantastical2.mac",
        "Week planning",
        None,
    ),
    window(("Notes", "com.apple.Notes"), "This week"),
    window(("Preview", "com.apple.Preview"), "invoice-october.pdf"),
];

const MEETING_WINDOWS: &[Window] = &[
    window(("zoom.us", "us.zoom.xos"), "Zoom Meeting"),
    window(SLACK, "#team"),
];

const WEB_WINDOWS: &[Window] = &[
    window(ZED, "checkout-form.tsx — northwind-web"),
    window(ZED, "address.ts — northwind-web"),
    page(
        "Checkout · localhost:5173",
        "http://localhost:5173/checkout",
    ),
    window(TERMINAL, "northwind-web — bun run dev"),
    window(
        ("Tower", "com.fournova.Tower3"),
        "northwind-web — feature/checkout",
    ),
];

const SUPPORT_WINDOWS: &[Window] = &[
    window(MAIL, "Re: Order export without VAT"),
    page(
        "Ticket #1042 · Helpdesk",
        "https://help.example.com/tickets/1042",
    ),
    window(SLACK, "#northwind-support"),
];

const IOS_WINDOWS: &[Window] = &[
    window(XCODE, "LumenWidget.swift — Lumen"),
    window(XCODE, "TimelineProvider.swift — Lumen"),
    page(
        "WidgetKit · Apple Developer",
        "https://developer.apple.com/documentation/widgetkit",
    ),
];

const DESIGN_WINDOWS: &[Window] = &[
    (
        "Affinity Designer",
        "com.seriflabs.affinitydesigner",
        "Onboarding.afdesign",
        None,
    ),
    (
        "Pixelmator Pro",
        "com.pixelmatorteam.pixelmator.x",
        "app-icons.pxd",
        None,
    ),
];

const API_WINDOWS: &[Window] = &[
    window(ZED, "rate_limit.rs — booking-api"),
    (
        "Postman",
        "com.postmanlabs.mac",
        "Partner API — Bookings",
        None,
    ),
    window(TERMINAL, "booking-api — cargo test"),
    (
        "Querious",
        "com.araeliumgroup.querious",
        "bookings — staging",
        None,
    ),
];

const WRITING_WINDOWS: &[Window] = &[
    ("iA Writer", "pro.writer.mac", "Local-first notes.md", None),
    page(
        "Local-first software",
        "https://www.inkandswitch.com/local-first/",
    ),
];

/// Short switches in the middle of a session.
pub(super) const INTERRUPTIONS: [Window; 2] =
    [window(SLACK, "Direct message"), window(MAIL, "Inbox")];

/// `(client and its accent, project)`; the internal project has a color of its own.
pub(super) const PROJECTS: [(Option<(&str, &str)>, &str); 5] = [
    (Some(("Northwind", "blue")), "Website relaunch"),
    (Some(("Northwind", "blue")), "Support"),
    (Some(("Lumen Labs", "purple")), "iOS app"),
    (Some(("Fernweh Travel", "teal")), "Booking API"),
    (None, "Internal"),
];
pub(super) const INTERNAL_COLOR: &str = "gold";

pub(super) const TAGS: [&str; 6] = [
    "meeting",
    "review",
    "bugfix",
    "planning",
    "design",
    "development",
];

/// One block of a day: `(start, end)` in minutes after midnight, or away.
pub(super) enum Block {
    Session {
        start: i64,
        end: i64,
        work: Work,
        project: usize,
        note: &'static str,
        tag: Option<usize>,
    },
    Away {
        start: i64,
        end: i64,
        reason: AwayReason,
    },
}

const fn session(
    (start, end): (i64, i64),
    work: Work,
    project: usize,
    note: &'static str,
    tag: Option<usize>,
) -> Block {
    Block::Session {
        start,
        end,
        work,
        project,
        note,
        tag,
    }
}

const fn away((start, end): (i64, i64), reason: AwayReason) -> Block {
    Block::Away { start, end, reason }
}

const fn hm(h: i64, m: i64) -> i64 {
    h * 60 + m
}

const MEETING: Option<usize> = Some(0);
const REVIEW: Option<usize> = Some(1);
const BUGFIX: Option<usize> = Some(2);
const PLANNING: Option<usize> = Some(3);
const DESIGN: Option<usize> = Some(4);
const DEVELOPMENT: Option<usize> = Some(5);

/// The days rotate through these schedules; the most recent day gets the first.
#[rustfmt::skip]
pub(super) const SCHEDULES: [&[Block]; 3] = [
    &[
        session((hm(8, 40), hm(9, 0)), Work::Admin, 4, "Inbox and week planning", PLANNING),
        session((hm(9, 0), hm(9, 20)), Work::Meeting, 0, "Standup", MEETING),
        session((hm(9, 20), hm(11, 45)), Work::Web, 0, "Checkout: address validation", DEVELOPMENT),
        session((hm(11, 45), hm(12, 30)), Work::Support, 1, "VAT missing in order export", BUGFIX),
        away((hm(12, 30), hm(13, 15)), AwayReason::Lock),
        session((hm(13, 15), hm(15, 0)), Work::Ios, 2, "Widget timeline refresh", DEVELOPMENT),
        away((hm(15, 0), hm(15, 10)), AwayReason::Idle),
        session((hm(15, 10), hm(16, 30)), Work::Api, 3, "Rate limits for partners", REVIEW),
        session((hm(16, 30), hm(17, 20)), Work::Writing, 4, "Blog post draft", None),
    ],
    &[
        session((hm(8, 50), hm(9, 10)), Work::Admin, 4, "Inbox", None),
        session((hm(9, 10), hm(9, 30)), Work::Meeting, 0, "Standup", MEETING),
        session((hm(9, 30), hm(10, 30)), Work::Design, 2, "Onboarding design review", DESIGN),
        session((hm(10, 30), hm(12, 0)), Work::Ios, 2, "Onboarding screens", DEVELOPMENT),
        away((hm(12, 0), hm(12, 45)), AwayReason::Lock),
        session((hm(12, 45), hm(14, 30)), Work::Api, 3, "Webhook retries", DEVELOPMENT),
        session((hm(14, 30), hm(15, 30)), Work::Web, 0, "Code review: product filters", REVIEW),
        away((hm(15, 30), hm(15, 45)), AwayReason::Idle),
        session((hm(15, 45), hm(17, 0)), Work::Support, 1, "Ticket backlog", None),
    ],
    &[
        session((hm(9, 0), hm(9, 30)), Work::Meeting, 2, "Sprint planning", PLANNING),
        session((hm(9, 30), hm(12, 15)), Work::Web, 0, "Product filters", DEVELOPMENT),
        away((hm(12, 15), hm(13, 0)), AwayReason::Lock),
        session((hm(13, 0), hm(14, 45)), Work::Api, 3, "Slow booking search", BUGFIX),
        session((hm(14, 45), hm(15, 45)), Work::Ios, 2, "TestFlight build", DEVELOPMENT),
        away((hm(15, 45), hm(16, 0)), AwayReason::Idle),
        session((hm(16, 0), hm(16, 45)), Work::Admin, 4, "Invoices", None),
    ],
];
