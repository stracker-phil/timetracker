use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::time::{Ms, now_ms};
use super::{Db, chosen_accent, name_in_use, name_taken, new_id, required_name};
use crate::error::{AppError, AppResult};
use crate::store::settings::ACCENTS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Client {
    pub id: String,
    pub name: String,
    /// An accent name (`ACCENTS`); the hue of the client's projects.
    pub color: String,
    pub archived_at: Option<Ms>,
    pub created_at: Ms,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ClientInput {
    pub name: String,
    /// Blank: a new client takes the first accent no active client has; an existing one keeps its color.
    pub color: String,
}

pub async fn list(db: &Db, include_archived: bool) -> AppResult<Vec<Client>> {
    Ok(sqlx::query_as(
        "SELECT * FROM clients WHERE ?1 OR archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )
    .bind(include_archived)
    .fetch_all(db.pool())
    .await?)
}

pub async fn get(db: &Db, id: &str) -> AppResult<Client> {
    found(
        id,
        sqlx::query_as("SELECT * FROM clients WHERE id = ?1")
            .bind(id)
            .fetch_optional(db.pool())
            .await?,
    )
}

pub async fn insert(db: &Db, input: ClientInput) -> AppResult<Client> {
    let name = unique_name(db, &input.name, None).await?;
    let color = match chosen_accent(&input.color)? {
        Some(color) => color,
        None => unused_color(db).await?,
    };
    Ok(sqlx::query_as(
        "INSERT INTO clients (id, name, color, created_at) VALUES (?1, ?2, ?3, ?4) RETURNING *",
    )
    .bind(new_id())
    .bind(name)
    .bind(color)
    .bind(now_ms())
    .fetch_one(db.pool())
    .await?)
}

/// A new color moves the client's projects to it.
pub async fn update(db: &Db, id: &str, input: ClientInput) -> AppResult<Client> {
    let existing = get(db, id).await?;
    let name = unique_name(db, &input.name, Some(id)).await?;
    let color = chosen_accent(&input.color)?.unwrap_or(existing.color);
    let mut tx = db.pool().begin().await?;
    let client = found(
        id,
        sqlx::query_as("UPDATE clients SET name = ?2, color = ?3 WHERE id = ?1 RETURNING *")
            .bind(id)
            .bind(name)
            .bind(&color)
            .fetch_optional(&mut *tx)
            .await?,
    )?;
    sqlx::query("UPDATE projects SET color = ?2 WHERE client_id = ?1")
        .bind(id)
        .bind(&color)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(client)
}

/// The first accent no active client has, else the first accent.
async fn unused_color(db: &Db) -> AppResult<String> {
    let clients = list(db, false).await?;
    let color = ACCENTS
        .iter()
        .find(|a| !clients.iter().any(|c| c.color == **a))
        .unwrap_or(&ACCENTS[0]);
    Ok(color.to_string())
}

/// The trimmed name, unless another client (archived ones too) has it.
async fn unique_name(db: &Db, name: &str, except: Option<&str>) -> AppResult<String> {
    let name = required_name(name)?;
    let clients = list(db, true).await?;
    if name_in_use(
        clients.iter().map(|c| (c.id.as_str(), c.name.as_str())),
        &name,
        except,
    ) {
        return Err(name_taken("A client"));
    }
    Ok(name)
}

/// Archiving also archives the client's active projects; unarchiving restores the client only.
pub async fn set_archived(db: &Db, id: &str, archived: bool) -> AppResult<Client> {
    let now = now_ms();
    let mut tx = db.pool().begin().await?;
    let client = found(
        id,
        sqlx::query_as(
            "UPDATE clients SET archived_at = CASE WHEN ?2 THEN coalesce(archived_at, ?3) END
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(archived)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?,
    )?;
    if archived {
        sqlx::query(
            "UPDATE projects SET archived_at = ?2 WHERE client_id = ?1 AND archived_at IS NULL",
        )
        .bind(id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(client)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Client>> {
    Ok(
        sqlx::query_as("SELECT * FROM clients ORDER BY created_at, id")
            .fetch_all(conn)
            .await?,
    )
}

fn found(id: &str, client: Option<Client>) -> AppResult<Client> {
    client.ok_or_else(|| AppError::NotFound(format!("No client {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    fn input(name: &str) -> ClientInput {
        colored(name, "")
    }

    fn colored(name: &str, color: &str) -> ClientInput {
        ClientInput {
            name: name.into(),
            color: color.into(),
        }
    }

    async fn add(db: &Db, name: &str) -> Client {
        insert(db, input(name)).await.unwrap()
    }

    fn color_error(err: AppError) -> String {
        match err {
            AppError::InvalidField { field, message } => {
                assert_eq!(field, "color");
                message
            }
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn insert_trims_the_name() {
        let db = Db::open_memory().await.unwrap();
        let client = add(&db, "  Acme  ").await;
        assert_eq!(client.name, "Acme");
        assert_eq!(client.archived_at, None);
        assert_eq!(get(&db, &client.id).await.unwrap(), client);
    }

    fn name_error(err: AppError) -> String {
        match err {
            AppError::InvalidField { field, message } => {
                assert_eq!(field, "name");
                message
            }
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    async fn project_of(db: &Db, client_id: &str, name: &str) -> crate::db::projects::Project {
        crate::db::projects::insert(
            db,
            crate::db::projects::ProjectInput {
                client_id: Some(client_id.into()),
                name: name.into(),
                color: "blue".into(),
                shade: None,
                billable: false,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn blank_names_are_rejected_on_the_name_field() {
        let db = Db::open_memory().await.unwrap();
        let existing = add(&db, "Acme").await;
        for name in ["", "   "] {
            let err = insert(&db, input(name)).await.unwrap_err();
            assert_eq!(name_error(err), "must not be empty", "insert {name:?}");
            let err = update(&db, &existing.id, input(name)).await.unwrap_err();
            assert_eq!(name_error(err), "must not be empty", "update {name:?}");
        }
    }

    #[tokio::test]
    async fn names_over_the_limit_are_rejected_and_the_limit_itself_is_fine() {
        let db = Db::open_memory().await.unwrap();
        let existing = add(&db, &"x".repeat(crate::db::MAX_NAME_CHARS)).await;
        let too_long = "y".repeat(crate::db::MAX_NAME_CHARS + 1);
        let err = insert(&db, input(&too_long)).await.unwrap_err();
        assert_eq!(name_error(err), "must be at most 100 characters");
        let err = update(&db, &existing.id, input(&too_long))
            .await
            .unwrap_err();
        assert_eq!(name_error(err), "must be at most 100 characters");
    }

    #[tokio::test]
    async fn names_count_characters_not_bytes() {
        let db = Db::open_memory().await.unwrap();
        let name = "ä".repeat(crate::db::MAX_NAME_CHARS);
        assert_eq!(add(&db, &name).await.name, name);
    }

    #[tokio::test]
    async fn a_name_another_client_has_is_refused_ignoring_case_and_archive_state() {
        let db = Db::open_memory().await.unwrap();
        add(&db, "Acme").await;
        add(&db, "Äpfel").await;
        let old = add(&db, "Globex").await;
        set_archived(&db, &old.id, true).await.unwrap();
        let other = add(&db, "Initech").await;

        for name in ["Acme", "acme", " ACME ", "äpfel", "ÄPFEL", "globex"] {
            let err = insert(&db, input(name)).await.unwrap_err();
            assert_eq!(
                name_error(err),
                "A client with this name already exists",
                "insert {name:?}"
            );
            let err = update(&db, &other.id, input(name)).await.unwrap_err();
            assert_eq!(
                name_error(err),
                "A client with this name already exists",
                "update {name:?}"
            );
        }
        assert_eq!(list(&db, true).await.unwrap().len(), 4);
    }

    #[tokio::test]
    async fn a_client_can_be_renamed_to_its_own_name_in_another_case() {
        let db = Db::open_memory().await.unwrap();
        let client = add(&db, "acme").await;
        let renamed = update(&db, &client.id, input("Acme")).await.unwrap();
        assert_eq!(renamed.name, "Acme");
        let again = update(&db, &client.id, input("Acme")).await.unwrap();
        assert_eq!(again.name, "Acme");
    }

    #[tokio::test]
    async fn unknown_ids_are_not_found() {
        let db = Db::open_memory().await.unwrap();
        assert_eq!(
            get(&db, "nope").await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            update(&db, "nope", input("x")).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
        assert_eq!(
            set_archived(&db, "nope", true).await.unwrap_err().kind(),
            ErrorKind::NotFound
        );
    }

    #[tokio::test]
    async fn update_renames_the_client() {
        let db = Db::open_memory().await.unwrap();
        let client = add(&db, "Acme").await;
        let renamed = update(&db, &client.id, input("Globex")).await.unwrap();
        assert_eq!(
            renamed,
            Client {
                name: "Globex".into(),
                ..client
            }
        );
    }

    #[tokio::test]
    async fn list_hides_archived_clients_unless_asked_and_sorts_ignoring_case() {
        let db = Db::open_memory().await.unwrap();
        add(&db, "banana").await;
        add(&db, "Cherry").await;
        add(&db, "apple").await;
        let old = add(&db, "Berry").await;
        set_archived(&db, &old.id, true).await.unwrap();

        let names = |clients: Vec<Client>| clients.into_iter().map(|c| c.name).collect::<Vec<_>>();
        assert_eq!(
            names(list(&db, false).await.unwrap()),
            ["apple", "banana", "Cherry"]
        );
        assert_eq!(
            names(list(&db, true).await.unwrap()),
            ["apple", "banana", "Berry", "Cherry"]
        );
    }

    #[tokio::test]
    async fn archiving_keeps_the_first_timestamp_and_unarchiving_clears_it() {
        let db = Db::open_memory().await.unwrap();
        let client = add(&db, "Acme").await;

        let first = set_archived(&db, &client.id, true).await.unwrap();
        let first_at = first.archived_at.expect("archived");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let second = set_archived(&db, &client.id, true).await.unwrap();
        assert_eq!(second.archived_at, Some(first_at));

        let restored = set_archived(&db, &client.id, false).await.unwrap();
        assert_eq!(restored.archived_at, None);
    }

    #[tokio::test]
    async fn archiving_a_client_archives_its_active_projects_only() {
        let db = Db::open_memory().await.unwrap();
        let client = add(&db, "Acme").await;
        let other_client = add(&db, "Globex").await;
        let active = project_of(&db, &client.id, "Active").await;
        let earlier = project_of(&db, &client.id, "Earlier").await;
        let elsewhere = project_of(&db, &other_client.id, "Elsewhere").await;
        let earlier_at = crate::db::projects::set_archived(&db, &earlier.id, true)
            .await
            .unwrap()
            .archived_at
            .expect("archived");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let archived = set_archived(&db, &client.id, true).await.unwrap();

        let get = |id: String| {
            let db = db.clone();
            async move { crate::db::projects::get(&db, &id).await.unwrap() }
        };
        assert_eq!(get(active.id).await.archived_at, archived.archived_at);
        assert_eq!(get(earlier.id).await.archived_at, Some(earlier_at));
        assert_eq!(get(elsewhere.id).await.archived_at, None);
    }

    #[tokio::test]
    async fn unarchiving_a_client_leaves_its_projects_archived() {
        let db = Db::open_memory().await.unwrap();
        let client = add(&db, "Acme").await;
        let project = project_of(&db, &client.id, "P").await;
        set_archived(&db, &client.id, true).await.unwrap();

        let restored = set_archived(&db, &client.id, false).await.unwrap();

        assert_eq!(restored.archived_at, None);
        assert!(
            crate::db::projects::get(&db, &project.id)
                .await
                .unwrap()
                .archived_at
                .is_some()
        );
    }

    #[tokio::test]
    async fn a_new_client_without_a_color_takes_the_first_accent_no_active_client_has() {
        let db = Db::open_memory().await.unwrap();
        let mut added = Vec::new();
        for name in ["A", "B", "C"] {
            added.push(add(&db, name).await);
        }
        let colors: Vec<&str> = added.iter().map(|c| c.color.as_str()).collect();
        assert_eq!(colors, ["orange", "gold", "red"]);

        let b = added.iter().find(|c| c.name == "B").unwrap();
        set_archived(&db, &b.id, true).await.unwrap();
        assert_eq!(add(&db, "D").await.color, "gold");
    }

    #[tokio::test]
    async fn a_blank_color_is_treated_like_none_and_wraps_to_the_first_accent_when_all_are_taken() {
        let db = Db::open_memory().await.unwrap();
        for (i, accent) in ACCENTS.iter().enumerate() {
            let client = insert(&db, colored(&format!("C{i}"), accent))
                .await
                .unwrap();
            assert_eq!(client.color, *accent);
        }
        let wrapped = insert(&db, colored("Extra", "   ")).await.unwrap();
        assert_eq!(wrapped.color, ACCENTS[0]);
    }

    #[tokio::test]
    async fn the_system_accent_is_refused_for_a_client() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, colored("Acme", "teal")).await.unwrap();

        let err = insert(&db, colored("Globex", "system")).await.unwrap_err();
        assert_eq!(color_error(err), "must be one of the accent colors");
        let err = update(&db, &client.id, colored("Acme", "system"))
            .await
            .unwrap_err();
        assert_eq!(color_error(err), "must be one of the accent colors");
        assert_eq!(list(&db, true).await.unwrap(), [client]);
    }

    #[tokio::test]
    async fn an_explicit_accent_is_stored_and_a_non_accent_is_refused() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, colored("Acme", "teal")).await.unwrap();
        assert_eq!(client.color, "teal");

        let err = insert(&db, colored("Globex", "chartreuse"))
            .await
            .unwrap_err();
        assert_eq!(color_error(err), "must be one of the accent colors");
        let err = update(&db, &client.id, colored("Acme", "chartreuse"))
            .await
            .unwrap_err();
        assert_eq!(color_error(err), "must be one of the accent colors");
        assert_eq!(list(&db, true).await.unwrap(), [client]);
    }

    #[tokio::test]
    async fn update_with_a_blank_color_keeps_the_color() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, colored("Acme", "purple")).await.unwrap();

        let renamed = update(&db, &client.id, colored("Globex", ""))
            .await
            .unwrap();

        assert_eq!(renamed.color, "purple");
        assert_eq!(renamed.name, "Globex");
    }

    #[tokio::test]
    async fn a_new_color_recolors_all_projects_of_the_client_and_no_others() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, colored("Acme", "teal")).await.unwrap();
        let other = insert(&db, colored("Globex", "red")).await.unwrap();
        let first = project_of(&db, &client.id, "First").await;
        let second = project_of(&db, &client.id, "Second").await;
        let archived = project_of(&db, &client.id, "Old").await;
        crate::db::projects::set_archived(&db, &archived.id, true)
            .await
            .unwrap();
        let elsewhere = project_of(&db, &other.id, "Elsewhere").await;

        let updated = update(&db, &client.id, colored("Acme", "indigo"))
            .await
            .unwrap();

        assert_eq!(updated.color, "indigo");
        for id in [&first.id, &second.id, &archived.id] {
            let project = crate::db::projects::get(&db, id).await.unwrap();
            assert_eq!(project.color, "indigo", "{}", project.name);
        }
        let untouched = crate::db::projects::get(&db, &elsewhere.id).await.unwrap();
        assert_eq!(untouched.color, "red");
        let kept = crate::db::projects::get(&db, &second.id).await.unwrap();
        assert_eq!(kept.shade, second.shade);
    }

    #[tokio::test]
    async fn a_client_serializes_its_color() {
        let db = Db::open_memory().await.unwrap();
        let client = insert(&db, colored("Acme", "gold")).await.unwrap();

        assert_eq!(
            serde_json::to_value(&client).unwrap(),
            serde_json::json!({
                "id": client.id,
                "name": "Acme",
                "color": "gold",
                "archivedAt": null,
                "createdAt": client.created_at,
            })
        );
    }
}
