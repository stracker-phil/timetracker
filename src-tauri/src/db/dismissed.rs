//! Dismissed suggestions: time of a day that a rule suggests nothing for again.

use sqlx::{Row, SqliteConnection};

use super::Db;
use super::time::{Day, Ms};
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dismissal {
    pub rule_id: String,
    pub day: Day,
    pub start: Ms,
    pub end: Ms,
}

pub async fn list_for_day(db: &Db, day: Day) -> AppResult<Vec<Dismissal>> {
    let rows = sqlx::query(
        r#"SELECT rule_id, start, "end" FROM dismissed_suggestions WHERE day = ?1 ORDER BY start"#,
    )
    .bind(day.to_string())
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(Dismissal {
                rule_id: row.try_get("rule_id")?,
                day,
                start: row.try_get("start")?,
                end: row.try_get("end")?,
            })
        })
        .collect()
}

/// Kept once; a rule deleted meanwhile leaves nothing to dismiss.
pub async fn put_on(conn: &mut SqliteConnection, dismissal: &Dismissal) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO dismissed_suggestions (rule_id, day, start, "end")
           SELECT ?1, ?2, ?3, ?4 WHERE EXISTS (SELECT 1 FROM rules WHERE id = ?1)
           ON CONFLICT DO NOTHING"#,
    )
    .bind(&dismissal.rule_id)
    .bind(dismissal.day.to_string())
    .bind(dismissal.start)
    .bind(dismissal.end)
    .execute(conn)
    .await?;
    Ok(())
}

pub async fn remove_on(conn: &mut SqliteConnection, dismissal: &Dismissal) -> AppResult<()> {
    sqlx::query(
        r#"DELETE FROM dismissed_suggestions WHERE rule_id = ?1 AND start = ?2 AND "end" = ?3"#,
    )
    .bind(&dismissal.rule_id)
    .bind(dismissal.start)
    .bind(dismissal.end)
    .execute(conn)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::store;
    use crate::rules::testing::save_rule;
    use crate::rules::types::{RuleField, RuleOp};
    use crate::test_support::project;

    fn day(s: &str) -> Day {
        Day::parse(s).unwrap()
    }

    fn dismissal(rule_id: &str, day: Day, start: Ms, end: Ms) -> Dismissal {
        Dismissal {
            rule_id: rule_id.into(),
            day,
            start,
            end,
        }
    }

    async fn put(db: &Db, dismissal: &Dismissal) {
        let mut conn = db.pool().acquire().await.unwrap();
        put_on(&mut conn, dismissal).await.unwrap();
    }

    async fn remove(db: &Db, dismissal: &Dismissal) {
        let mut conn = db.pool().acquire().await.unwrap();
        remove_on(&mut conn, dismissal).await.unwrap();
    }

    async fn a_rule(db: &Db) -> String {
        let project = project(db, "Portal").await;
        let conditions = [(RuleField::App, RuleOp::Is, "Slack")];
        save_rule(db, "Slack", &project.id, &conditions).await.id
    }

    #[tokio::test]
    async fn a_dismissal_is_listed_for_its_day_by_start() {
        let db = Db::open_memory().await.unwrap();
        let rule_id = a_rule(&db).await;
        let (monday, tuesday) = (day("2026-01-05"), day("2026-01-06"));
        let late = dismissal(&rule_id, monday, 3_000, 4_000);
        let early = dismissal(&rule_id, monday, 1_000, 2_000);
        let other_day = dismissal(&rule_id, tuesday, 90_000_000, 91_000_000);
        for d in [&late, &early, &other_day] {
            put(&db, d).await;
        }

        assert_eq!(list_for_day(&db, monday).await.unwrap(), [early, late]);
        assert_eq!(list_for_day(&db, tuesday).await.unwrap(), [other_day]);
        assert!(
            list_for_day(&db, day("2026-01-07"))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn dismissing_the_same_span_twice_keeps_one() {
        let db = Db::open_memory().await.unwrap();
        let rule_id = a_rule(&db).await;
        let monday = day("2026-01-05");
        let d = dismissal(&rule_id, monday, 1_000, 2_000);

        put(&db, &d).await;
        put(&db, &d).await;

        assert_eq!(list_for_day(&db, monday).await.unwrap(), [d]);
    }

    #[tokio::test]
    async fn dismissing_for_a_rule_that_is_gone_writes_nothing() {
        let db = Db::open_memory().await.unwrap();
        let monday = day("2026-01-05");

        put(&db, &dismissal("gone", monday, 1_000, 2_000)).await;

        assert!(list_for_day(&db, monday).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn removing_a_dismissal_leaves_the_others() {
        let db = Db::open_memory().await.unwrap();
        let rule_id = a_rule(&db).await;
        let monday = day("2026-01-05");
        let first = dismissal(&rule_id, monday, 1_000, 2_000);
        let second = dismissal(&rule_id, monday, 3_000, 4_000);
        put(&db, &first).await;
        put(&db, &second).await;

        remove(&db, &first).await;

        assert_eq!(list_for_day(&db, monday).await.unwrap(), [second]);
    }

    #[tokio::test]
    async fn deleting_a_rule_deletes_its_dismissals() {
        let db = Db::open_memory().await.unwrap();
        let rule_id = a_rule(&db).await;
        let monday = day("2026-01-05");
        put(&db, &dismissal(&rule_id, monday, 1_000, 2_000)).await;

        store::delete(&db, &rule_id).await.unwrap();

        assert!(list_for_day(&db, monday).await.unwrap().is_empty());
    }
}
