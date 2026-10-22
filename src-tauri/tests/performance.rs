//! The views stay fast on two years of seeded data (`bun run test:db`). Thresholds are medians.

use std::future::Future;
use std::time::{Duration, Instant};

use chrono::{Datelike, Days};
use timetracker_lib::calendar::commands::{load_month, load_week};
use timetracker_lib::calendar::mini_month::{Shown, load_mini_month};
use timetracker_lib::calendar::period::PeriodUnit;
use timetracker_lib::day::list::list;
use timetracker_lib::day::{load, view_of};
use timetracker_lib::db::time::{Day, now_ms};
use timetracker_lib::reports::{self, Range, ReportFilter};
use timetracker_lib::store::settings::{CalendarTitle, Settings};
use timetracker_lib::test_support::seed::{ACTIVITIES_PER_DAY, seed_years};
use timetracker_lib::test_support::temp_db;

const RUNS: usize = 20;

async fn median<F, Fut, T>(mut run: F) -> Duration
where
    F: FnMut() -> Fut,
    Fut: Future<Output = T>,
{
    let mut times = Vec::with_capacity(RUNS);
    for _ in 0..RUNS {
        let started = Instant::now();
        std::hint::black_box(run().await);
        times.push(started.elapsed());
    }
    times.sort();
    times[RUNS / 2]
}

#[tokio::test]
#[ignore]
async fn views_stay_fast_on_two_years_of_data() {
    let (_dir, db) = temp_db().await;
    let started = Instant::now();
    let stats = seed_years(&db, 2, 42).await.unwrap();
    println!("seeded {stats:?} in {:?}", started.elapsed());

    let today = Day::today();
    let mut day = Day::from_date(today.date() - Days::new(365));
    while day.date().weekday().number_from_monday() > 5 {
        day = day.prev();
    }
    let settings = Settings::default();
    let now = now_ms();
    let view = load(&db, day, &settings, now).await.unwrap();
    assert!(
        view.lanes.iter().map(|l| l.blocks.len()).sum::<usize>() > 0,
        "the seeded day {day} shows no activity"
    );
    assert!(stats.activities >= 500 * ACTIVITIES_PER_DAY as u64);

    let month = ReportFilter {
        range: Range::Custom {
            from: day.month_start(),
            to: Day::from_date(day.month_start().date() + chrono::Months::new(1)).prev(),
        },
        ..ReportFilter::default()
    };
    let measured = [
        (
            "day view",
            median(|| load(&db, day, &settings, now)).await,
            50,
        ),
        (
            "report for a month",
            median(|| reports::run(&db, &month, today, now)).await,
            200,
        ),
        ("week view", median(|| load_week(&db, day)).await, 100),
        ("month view", median(|| load_month(&db, day)).await, 150),
        (
            "sidebar calendar",
            median(|| {
                let shown = Shown::Period {
                    unit: PeriodUnit::Day,
                    day,
                };
                load_mini_month(Some(&db), day, 0, shown, CalendarTitle::Short, today, now)
            })
            .await,
            20,
        ),
        (
            "list view",
            median(|| async { list(&view_of(&db, day, &settings, now).await.unwrap()) }).await,
            50,
        ),
    ];

    let report: Vec<String> = measured
        .iter()
        .map(|(name, took, limit)| format!("{name}: {took:?} (limit {limit} ms)"))
        .collect();
    println!("{}", report.join("\n"));
    let slow: Vec<&String> = measured
        .iter()
        .zip(&report)
        .filter(|((_, took, limit), _)| took.as_millis() >= *limit)
        .map(|(_, line)| line)
        .collect();
    assert!(
        slow.is_empty(),
        "too slow (median of {RUNS} runs):\n{slow:#?}"
    );
}
