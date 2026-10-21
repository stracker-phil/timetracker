//! Rules and their conditions in SQLite, in position order.

use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Row, SqliteConnection};

use super::types::{Condition, ConditionInput, Rule, RuleInput, RuleOp};
use crate::db::{Db, new_id, projects, required_name, tags};
use crate::error::{AppError, AppResult};

const MAX_NOTE_CHARS: usize = 500;

struct RuleRow {
    id: String,
    name: String,
    project_id: String,
    position: i64,
    enabled: bool,
    note: String,
    tag_ids: Vec<String>,
}

impl FromRow<'_, SqliteRow> for RuleRow {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            project_id: row.try_get("project_id")?,
            position: row.try_get("position")?,
            enabled: row.try_get("enabled")?,
            note: row.try_get("note")?,
            tag_ids: decoded(row, "tag_ids", true)?,
        })
    }
}

/// A JSON column, or an enum stored as its serde name.
fn decoded<T: serde::de::DeserializeOwned>(
    row: &SqliteRow,
    column: &str,
    json: bool,
) -> sqlx::Result<T> {
    let text: String = row.try_get(column)?;
    let value = if json {
        serde_json::from_str(&text)
    } else {
        serde_json::from_value(serde_json::Value::String(text))
    };
    value.map_err(|e| sqlx::Error::ColumnDecode {
        index: column.into(),
        source: Box::new(e),
    })
}

impl FromRow<'_, SqliteRow> for Condition {
    fn from_row(row: &SqliteRow) -> sqlx::Result<Self> {
        Ok(Self {
            id: row.try_get("id")?,
            field: decoded(row, "field", false)?,
            op: decoded(row, "op", false)?,
            value: row.try_get("value")?,
        })
    }
}

/// The serde name of a field or op, as stored.
fn stored<T: serde::Serialize>(value: &T) -> AppResult<String> {
    Ok(serde_json::to_value(value)?
        .as_str()
        .unwrap_or_default()
        .to_string())
}

fn sql_position(index: usize) -> i64 {
    i64::try_from(index).unwrap_or(i64::MAX)
}

fn invalid(field: impl Into<String>, message: &str) -> AppError {
    AppError::InvalidField {
        field: field.into(),
        message: message.into(),
    }
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound(format!("No rule {id}"))
}

pub async fn list(db: &Db) -> AppResult<Vec<Rule>> {
    let mut conn = db.pool().acquire().await?;
    all(&mut conn).await
}

pub async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Rule>> {
    let rows: Vec<RuleRow> = sqlx::query_as("SELECT * FROM rules ORDER BY position, id")
        .fetch_all(&mut *conn)
        .await?;
    let conditions: Vec<(String, Condition)> =
        sqlx::query("SELECT * FROM rule_conditions ORDER BY rule_id, position")
            .fetch_all(&mut *conn)
            .await?
            .iter()
            .map(|row| Ok((row.try_get("rule_id")?, Condition::from_row(row)?)))
            .collect::<sqlx::Result<_>>()?;
    Ok(rows
        .into_iter()
        .map(|row| Rule {
            conditions: conditions
                .iter()
                .filter(|(rule_id, _)| *rule_id == row.id)
                .map(|(_, c)| c.clone())
                .collect(),
            id: row.id,
            name: row.name,
            project_id: row.project_id,
            position: row.position,
            enabled: row.enabled,
            note: row.note,
            tag_ids: row.tag_ids,
        })
        .collect())
}

/// The rules suggestions come from: enabled, with a project that isn't archived.
pub async fn active(db: &Db) -> AppResult<Vec<Rule>> {
    let open: Vec<String> = projects::list(db, false)
        .await?
        .into_iter()
        .map(|p| p.id)
        .collect();
    Ok(list(db)
        .await?
        .into_iter()
        .filter(|r| r.enabled && open.contains(&r.project_id))
        .collect())
}

pub async fn get(db: &Db, id: &str) -> AppResult<Rule> {
    list(db)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or_else(|| not_found(id))
}

/// `input` with its name and note trimmed, or the first field it gets wrong.
async fn validated(db: &Db, input: &RuleInput) -> AppResult<RuleInput> {
    let name = required_name(&input.name)?;
    let note = input.note.trim();
    if note.contains(['\n', '\r']) {
        return Err(invalid("note", "must be a single line"));
    }
    if note.chars().count() > MAX_NOTE_CHARS {
        return Err(invalid(
            "note",
            &format!("must be at most {MAX_NOTE_CHARS} characters"),
        ));
    }
    let project = projects::get(db, &input.project_id)
        .await
        .map_err(|_| invalid("projectId", "is not a project"))?;
    if project.archived_at.is_some() {
        return Err(invalid("projectId", "is archived"));
    }
    let known = tags::list(db).await?;
    if !input
        .tag_ids
        .iter()
        .all(|id| known.iter().any(|t| &t.id == id))
    {
        return Err(invalid("tagIds", "has an unknown tag"));
    }
    check_conditions(&input.conditions)?;
    Ok(RuleInput {
        name,
        note: note.to_string(),
        ..input.clone()
    })
}

/// At least one; each with a value, except `is ""`.
pub fn check_conditions(conditions: &[ConditionInput]) -> AppResult<()> {
    if conditions.is_empty() {
        return Err(invalid("conditions", "needs at least one condition"));
    }
    for (index, condition) in conditions.iter().enumerate() {
        if condition.op != RuleOp::Is && condition.value.trim().is_empty() {
            return Err(invalid(
                format!("conditions.{index}.value"),
                "must not be empty",
            ));
        }
    }
    Ok(())
}

/// A new rule (`id` `None`) goes last; an existing one keeps its position and gets the new conditions.
pub async fn save(db: &Db, id: Option<&str>, input: &RuleInput) -> AppResult<Rule> {
    let input = validated(db, input).await?;
    let mut tx = db.pool().begin().await?;
    let id = write_rule(&mut tx, id, &input).await?;
    write_conditions(&mut tx, &id, &input.conditions).await?;
    tx.commit().await?;
    get(db, &id).await
}

/// Updates the rule `id`, or inserts one when `None`; returns its id.
async fn write_rule(
    conn: &mut SqliteConnection,
    id: Option<&str>,
    input: &RuleInput,
) -> AppResult<String> {
    let tag_ids = serde_json::to_string(&input.tag_ids)?;
    let Some(id) = id else {
        let id = new_id();
        sqlx::query(
            "INSERT INTO rules (id, name, project_id, position, enabled, note, tag_ids)
             VALUES (?1, ?2, ?3, (SELECT coalesce(max(position), -1) + 1 FROM rules), ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(&input.name)
        .bind(&input.project_id)
        .bind(input.enabled)
        .bind(&input.note)
        .bind(&tag_ids)
        .execute(&mut *conn)
        .await?;
        return Ok(id);
    };
    let changed = sqlx::query(
        "UPDATE rules SET name = ?2, project_id = ?3, enabled = ?4, note = ?5, tag_ids = ?6 WHERE id = ?1",
    )
    .bind(id)
    .bind(&input.name)
    .bind(&input.project_id)
    .bind(input.enabled)
    .bind(&input.note)
    .bind(&tag_ids)
    .execute(&mut *conn)
    .await?
    .rows_affected();
    if changed == 0 {
        return Err(not_found(id));
    }
    Ok(id.to_string())
}

/// Replaces the rule's conditions, keeping their order.
async fn write_conditions(
    conn: &mut SqliteConnection,
    rule_id: &str,
    conditions: &[ConditionInput],
) -> AppResult<()> {
    sqlx::query("DELETE FROM rule_conditions WHERE rule_id = ?1")
        .bind(rule_id)
        .execute(&mut *conn)
        .await?;
    for (index, condition) in conditions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO rule_conditions (id, rule_id, position, field, op, value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(new_id())
        .bind(rule_id)
        .bind(sql_position(index))
        .bind(stored(&condition.field)?)
        .bind(stored(&condition.op)?)
        .bind(&condition.value)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn delete(db: &Db, id: &str) -> AppResult<()> {
    let deleted = sqlx::query("DELETE FROM rules WHERE id = ?1")
        .bind(id)
        .execute(db.pool())
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(not_found(id));
    }
    Ok(())
}

/// `ids` is every rule's id in the new order.
pub async fn reorder(db: &Db, ids: &[String]) -> AppResult<Vec<Rule>> {
    let mut current: Vec<String> = list(db).await?.into_iter().map(|r| r.id).collect();
    let mut wanted = ids.to_vec();
    current.sort();
    wanted.sort();
    if current != wanted {
        return Err(AppError::InvalidInput(
            "The order must list every rule once".into(),
        ));
    }
    let mut tx = db.pool().begin().await?;
    for (position, id) in ids.iter().enumerate() {
        sqlx::query("UPDATE rules SET position = ?2 WHERE id = ?1")
            .bind(id)
            .bind(sql_position(position))
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    list(db).await
}

pub async fn set_enabled(db: &Db, id: &str, enabled: bool) -> AppResult<Rule> {
    let changed = sqlx::query("UPDATE rules SET enabled = ?2 WHERE id = ?1")
        .bind(id)
        .bind(enabled)
        .execute(db.pool())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(not_found(id));
    }
    get(db, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::projects::Project;
    use crate::db::tags::Tag;
    use crate::error::ErrorKind;
    use crate::rules::testing::*;
    use crate::rules::types::RuleField::*;
    use crate::test_support::project;

    struct World {
        db: Db,
        project: Project,
        tag: Tag,
    }

    impl World {
        async fn new() -> Self {
            let db = Db::open_memory().await.unwrap();
            let project = project(&db, "Portal").await;
            let tag = tags::insert(&db, "billing").await.unwrap();
            Self { db, project, tag }
        }

        async fn save(&self, name: &str) -> Rule {
            save_rule(&self.db, name, &self.project.id, &[(App, RuleOp::Is, name)]).await
        }

        fn input(&self) -> RuleInput {
            rule_input("Slack", &self.project.id, &[(App, RuleOp::Is, "Slack")])
        }

        fn edited(&self, edit: impl FnOnce(&mut RuleInput)) -> RuleInput {
            let mut input = self.input();
            edit(&mut input);
            input
        }

        async fn ids(&self) -> Vec<String> {
            list(&self.db)
                .await
                .unwrap()
                .into_iter()
                .map(|r| r.id)
                .collect()
        }

        async fn condition_rows(&self) -> i64 {
            sqlx::query_scalar("SELECT COUNT(*) FROM rule_conditions")
                .fetch_one(self.db.pool())
                .await
                .unwrap()
        }
    }

    #[tokio::test]
    async fn a_saved_rule_is_stored_with_its_conditions_in_order() {
        let w = World::new().await;
        let input = RuleInput {
            note: "standup".into(),
            tag_ids: vec![w.tag.id.clone()],
            ..rule_input(
                "Slack standup",
                &w.project.id,
                &[
                    (App, RuleOp::Is, "Slack"),
                    (Title, RuleOp::Contains, "standup"),
                    (Domain, RuleOp::Pattern, "*.example.com"),
                ],
            )
        };

        let saved = save(&w.db, None, &input).await.unwrap();

        assert_eq!(
            (
                saved.name.as_str(),
                saved.project_id.as_str(),
                saved.position,
                saved.enabled,
                saved.note.as_str(),
                saved.tag_ids.clone(),
            ),
            (
                "Slack standup",
                w.project.id.as_str(),
                0,
                true,
                "standup",
                vec![w.tag.id.clone()]
            )
        );
        assert_eq!(
            specs(&saved),
            [
                (App, RuleOp::Is, "Slack".to_string()),
                (Title, RuleOp::Contains, "standup".to_string()),
                (Domain, RuleOp::Pattern, "*.example.com".to_string()),
            ]
        );
        assert_eq!(list(&w.db).await.unwrap(), [saved.clone()]);
        assert_eq!(get(&w.db, &saved.id).await.unwrap(), saved);
    }

    #[tokio::test]
    async fn name_and_note_are_trimmed_and_an_empty_is_value_is_allowed() {
        let w = World::new().await;
        let input = RuleInput {
            name: "  Slack  ".into(),
            note: "  daily ".into(),
            ..rule_input("", &w.project.id, &[(Title, RuleOp::Is, "")])
        };

        let saved = save(&w.db, None, &input).await.unwrap();

        assert_eq!(
            (saved.name.as_str(), saved.note.as_str()),
            ("Slack", "daily")
        );
        assert_eq!(specs(&saved), [(Title, RuleOp::Is, String::new())]);
    }

    #[tokio::test]
    async fn new_rules_go_after_the_last_position_even_when_earlier_ones_were_deleted() {
        let w = World::new().await;
        let first = w.save("A").await;
        let second = w.save("B").await;
        delete(&w.db, &first.id).await.unwrap();

        let third = w.save("C").await;

        assert_eq!((first.position, second.position, third.position), (0, 1, 2));
        assert_eq!(w.ids().await, [second.id, third.id]);
    }

    #[tokio::test]
    async fn updating_keeps_the_position_and_replaces_the_conditions() {
        let w = World::new().await;
        let first = w.save("A").await;
        let second = w.save("B").await;
        let input = RuleInput {
            enabled: false,
            ..rule_input(
                "Renamed",
                &w.project.id,
                &[
                    (Title, RuleOp::BeginsWith, "x"),
                    (Path, RuleOp::Contains, "y"),
                ],
            )
        };

        let updated = save(&w.db, Some(&first.id), &input).await.unwrap();

        assert_eq!(
            (
                updated.id.as_str(),
                updated.name.as_str(),
                updated.position,
                updated.enabled
            ),
            (first.id.as_str(), "Renamed", 0, false)
        );
        assert_eq!(
            specs(&updated),
            [
                (Title, RuleOp::BeginsWith, "x".to_string()),
                (Path, RuleOp::Contains, "y".to_string()),
            ]
        );
        assert_eq!(w.ids().await, [first.id, second.id]);
        assert_eq!(w.condition_rows().await, 3);
    }

    #[tokio::test]
    async fn updating_an_unknown_rule_is_not_found() {
        let w = World::new().await;

        let err = save(&w.db, Some("nope"), &w.input()).await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
        assert!(list(&w.db).await.unwrap().is_empty());
        assert_eq!(w.condition_rows().await, 0);
    }

    #[tokio::test]
    async fn invalid_rules_are_refused_naming_the_field_and_save_nothing() {
        let w = World::new().await;
        let archived = project(&w.db, "Old").await;
        projects::set_archived(&w.db, &archived.id, true)
            .await
            .unwrap();
        let condition = |value: &str| ConditionInput {
            field: App,
            op: RuleOp::Contains,
            value: value.into(),
        };
        let cases = [
            ("a blank name", w.edited(|i| i.name = "  ".into()), "name"),
            (
                "a note with a line break",
                w.edited(|i| i.note = "a\nb".into()),
                "note",
            ),
            (
                "a note of 501 characters",
                w.edited(|i| i.note = "x".repeat(501)),
                "note",
            ),
            (
                "an unknown project",
                w.edited(|i| i.project_id = "nope".into()),
                "projectId",
            ),
            (
                "an archived project",
                w.edited(|i| i.project_id = archived.id.clone()),
                "projectId",
            ),
            (
                "an unknown tag",
                w.edited(|i| i.tag_ids = vec!["nope".into()]),
                "tagIds",
            ),
            (
                "no conditions",
                w.edited(|i| i.conditions = vec![]),
                "conditions",
            ),
            (
                "an empty value",
                w.edited(|i| i.conditions = vec![condition("")]),
                "conditions.0.value",
            ),
            (
                "a blank value",
                w.edited(|i| i.conditions = vec![condition("  ")]),
                "conditions.0.value",
            ),
            (
                "an empty value in the second condition",
                w.edited(|i| i.conditions.push(condition(""))),
                "conditions.1.value",
            ),
        ];

        for (name, input, field) in cases {
            let err = save(&w.db, None, &input).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name}");
            assert_eq!(field_of(&err).as_deref(), Some(field), "{name}");
        }

        assert!(list(&w.db).await.unwrap().is_empty());
        assert_eq!(w.condition_rows().await, 0);
    }

    #[tokio::test]
    async fn deleting_a_rule_removes_its_conditions_only() {
        let w = World::new().await;
        let first = w.save("A").await;
        let second = w.save("B").await;
        assert_eq!(w.condition_rows().await, 2);

        delete(&w.db, &first.id).await.unwrap();

        assert_eq!(w.ids().await, [second.id]);
        assert_eq!(w.condition_rows().await, 1);
    }

    #[tokio::test]
    async fn deleting_an_unknown_rule_is_not_found() {
        let w = World::new().await;

        let err = delete(&w.db, "nope").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn reorder_applies_the_new_order() {
        let w = World::new().await;
        let (a, b, c) = (w.save("A").await, w.save("B").await, w.save("C").await);
        let order = vec![c.id.clone(), a.id.clone(), b.id.clone()];

        let reordered = reorder(&w.db, &order).await.unwrap();

        let shape: Vec<(String, i64)> = reordered.into_iter().map(|r| (r.id, r.position)).collect();
        assert_eq!(shape, [(c.id, 0), (a.id, 1), (b.id, 2)]);
        assert_eq!(w.ids().await, order);
    }

    #[tokio::test]
    async fn reorder_refuses_a_list_that_is_not_every_rule_once() {
        let w = World::new().await;
        let (a, b) = (w.save("A").await, w.save("B").await);
        let cases = [
            ("a partial list", vec![b.id.clone()]),
            ("a duplicate", vec![a.id.clone(), a.id.clone()]),
            (
                "an extra id",
                vec![a.id.clone(), b.id.clone(), "nope".into()],
            ),
            ("an unknown id", vec![a.id.clone(), "nope".into()]),
            ("an empty list", vec![]),
        ];

        for (name, ids) in cases {
            let err = reorder(&w.db, &ids).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{name}");
        }

        assert_eq!(w.ids().await, [a.id, b.id]);
    }

    #[tokio::test]
    async fn set_enabled_switches_a_rule_and_returns_it() {
        let w = World::new().await;
        let saved = w.save("A").await;

        let off = set_enabled(&w.db, &saved.id, false).await.unwrap();
        let on = set_enabled(&w.db, &saved.id, true).await.unwrap();

        assert_eq!((off.enabled, on.enabled), (false, true));
        assert_eq!(
            Rule {
                enabled: true,
                ..off
            },
            on
        );
        let err = set_enabled(&w.db, "nope", true).await.unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn get_of_an_unknown_rule_is_not_found() {
        let w = World::new().await;

        let err = get(&w.db, "nope").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn active_rules_are_enabled_and_belong_to_an_open_project() {
        let w = World::new().await;
        let old = project(&w.db, "Old").await;
        let open = w.save("Open").await;
        let off = w.save("Off").await;
        set_enabled(&w.db, &off.id, false).await.unwrap();
        save_rule(&w.db, "Archived", &old.id, &[(App, RuleOp::Is, "x")]).await;
        projects::set_archived(&w.db, &old.id, true).await.unwrap();

        let active = active(&w.db).await.unwrap();

        assert_eq!(
            active.into_iter().map(|r| r.id).collect::<Vec<_>>(),
            [open.id]
        );
    }
}
