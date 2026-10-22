use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;

use super::clients::{self, Client};
use super::time::{Ms, now_ms};
use super::{Db, chosen_accent, invalid_field, name_in_use, new_id, required_name, same_name};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Project {
    pub id: String,
    pub client_id: Option<String>,
    pub name: String,
    /// An accent name (`ACCENTS`): the client's color, or the project's own without a client.
    pub color: String,
    /// The lightness step of `color`, `0..SHADES`, darkest first.
    pub shade: u8,
    /// Its recordings count as billable time.
    pub billable: bool,
    pub archived_at: Option<Ms>,
    pub created_at: Ms,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ProjectInput {
    pub client_id: Option<String>,
    pub name: String,
    /// Ignored when the project has a client: it takes the client's color.
    pub color: String,
    /// `None`: kept on an edit within the same client, else the next shade no sibling has.
    pub shade: Option<u8>,
    pub billable: bool,
}

/// A client-less project's color when none is given.
pub const DEFAULT_COLOR: &str = "orange";
/// How many shades a color has.
const SHADES: u8 = 9;
/// The order new projects of a client get shades in: far apart first.
const SHADE_ORDER: [u8; SHADES as usize] = [4, 1, 7, 2, 6, 0, 8, 3, 5];

struct Stored {
    client_id: Option<String>,
    name: String,
    color: String,
    shade: u8,
    billable: bool,
}

/// The input as stored. `existing` is the project being edited; its client may stay archived.
async fn validated(db: &Db, input: ProjectInput, existing: Option<&Project>) -> AppResult<Stored> {
    let name = required_name(&input.name)?;
    let client_changed = existing.is_none_or(|p| p.client_id != input.client_id);
    let color = color_of(db, &input, client_changed).await?;
    let projects = list(db, true).await?;
    let except = existing.map(|e| e.id.as_str());
    let siblings: Vec<&Project> = projects
        .iter()
        .filter(|p| p.client_id == input.client_id && Some(p.id.as_str()) != except)
        .collect();
    let shade = shade_of(&input, existing.filter(|_| !client_changed), &siblings)?;
    if name_in_use(
        siblings.iter().map(|p| (p.id.as_str(), p.name.as_str())),
        &name,
        None,
    ) {
        let owner = if input.client_id.is_some() {
            "of this client"
        } else {
            "without a client"
        };
        return Err(invalid_field(
            "name",
            format!("is used by another project {owner}"),
        ));
    }
    Ok(Stored {
        client_id: input.client_id,
        name,
        color,
        shade,
        billable: input.billable,
    })
}

/// The client's color, or the input's for a project without a client.
async fn color_of(db: &Db, input: &ProjectInput, client_changed: bool) -> AppResult<String> {
    let Some(client_id) = input.client_id.as_deref() else {
        return Ok(chosen_accent(&input.color)?.unwrap_or_else(|| DEFAULT_COLOR.to_string()));
    };
    let client = clients::get(db, client_id).await?;
    if client_changed && client.archived_at.is_some() {
        return Err(invalid_field("clientId", "is archived"));
    }
    Ok(client.color)
}

/// The input's shade; without one, `same_client`'s (the edited project when its client stays), else
/// the next one no sibling has.
fn shade_of(
    input: &ProjectInput,
    same_client: Option<&Project>,
    siblings: &[&Project],
) -> AppResult<u8> {
    match (input.shade, same_client) {
        (Some(shade), _) if shade < SHADES => Ok(shade),
        (Some(_), _) => Err(invalid_field(
            "shade",
            format!("must be between 0 and {}", SHADES - 1),
        )),
        (None, Some(project)) => Ok(project.shade),
        (None, None) => Ok(next_shade(siblings.iter().map(|p| p.shade))),
    }
}

/// The first shade of `SHADE_ORDER` no sibling has; when all are taken, the order starts over.
fn next_shade(taken: impl Iterator<Item = u8>) -> u8 {
    let taken: Vec<u8> = taken.collect();
    SHADE_ORDER
        .iter()
        .copied()
        .find(|s| !taken.contains(s))
        .unwrap_or(SHADE_ORDER[taken.len() % SHADE_ORDER.len()])
}

pub async fn list(db: &Db, include_archived: bool) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as(
        "SELECT * FROM projects WHERE ?1 OR archived_at IS NULL ORDER BY name COLLATE NOCASE",
    )
    .bind(include_archived)
    .fetch_all(db.pool())
    .await?)
}

pub async fn get(db: &Db, id: &str) -> AppResult<Project> {
    found(
        id,
        sqlx::query_as("SELECT * FROM projects WHERE id = ?1")
            .bind(id)
            .fetch_optional(db.pool())
            .await?,
    )
}

pub async fn insert(db: &Db, input: ProjectInput) -> AppResult<Project> {
    let input = validated(db, input, None).await?;
    Ok(sqlx::query_as(
        "INSERT INTO projects (id, client_id, name, color, shade, billable, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING *",
    )
    .bind(new_id())
    .bind(input.client_id)
    .bind(input.name)
    .bind(input.color)
    .bind(input.shade)
    .bind(input.billable)
    .bind(now_ms())
    .fetch_one(db.pool())
    .await?)
}

/// Keeps the archive state.
pub async fn update(db: &Db, id: &str, input: ProjectInput) -> AppResult<Project> {
    let existing = get(db, id).await?;
    let input = validated(db, input, Some(&existing)).await?;
    found(
        id,
        sqlx::query_as(
            "UPDATE projects SET client_id = ?2, name = ?3, color = ?4, shade = ?5, billable = ?6
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(input.client_id)
        .bind(input.name)
        .bind(input.color)
        .bind(input.shade)
        .bind(input.billable)
        .fetch_optional(db.pool())
        .await?,
    )
}

/// Archiving keeps the first archive time; unarchiving clears it.
pub async fn set_archived(db: &Db, id: &str, archived: bool) -> AppResult<Project> {
    found(
        id,
        sqlx::query_as(
            "UPDATE projects SET archived_at = CASE WHEN ?2 THEN coalesce(archived_at, ?3) END
             WHERE id = ?1 RETURNING *",
        )
        .bind(id)
        .bind(archived)
        .bind(now_ms())
        .fetch_optional(db.pool())
        .await?,
    )
}

/// The project an id or a name stands for: an exact id; else an active project with exactly that name,
/// ignoring case; else the one active project whose name starts with it. Several matches are
/// `invalid_input` naming them.
pub async fn resolve(db: &Db, name_or_id: &str) -> AppResult<Project> {
    let text = name_or_id.trim();
    if let Some(project) = sqlx::query_as("SELECT * FROM projects WHERE id = ?1")
        .bind(text)
        .fetch_optional(db.pool())
        .await?
    {
        return Ok(project);
    }
    let active = list(db, false).await?;
    let exact: Vec<Project> = active
        .iter()
        .filter(|p| same_name(&p.name, text))
        .cloned()
        .collect();
    let candidates = if exact.is_empty() {
        let prefix = text.to_lowercase();
        active
            .into_iter()
            .filter(|p| !prefix.is_empty() && p.name.to_lowercase().starts_with(&prefix))
            .collect()
    } else {
        exact
    };
    match candidates.as_slice() {
        [] => Err(AppError::NotFound(format!("No project matches `{text}`"))),
        [project] => Ok(project.clone()),
        several => Err(AppError::InvalidInput(format!(
            "`{text}` matches several projects: {}",
            several
                .iter()
                .map(|p| format!("{} ({})", p.name, p.id))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Active projects by their latest recording, the most recent first; projects never recorded are left out.
pub async fn recent(db: &Db, limit: u32) -> AppResult<Vec<Project>> {
    Ok(sqlx::query_as(
        "SELECT p.* FROM projects p JOIN recordings r ON r.project_id = p.id
         JOIN recording_spans s ON s.recording_id = r.id
         WHERE p.archived_at IS NULL GROUP BY p.id ORDER BY max(s.start) DESC, p.name LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(db.pool())
    .await?)
}

/// A project as the Projects screen lists it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ProjectListItem {
    pub project: Project,
    pub client: Option<Client>,
    /// The start of its latest recording span.
    pub last_used: Option<Ms>,
    /// Recorded time in the 30 days before now; a running recording counts until now.
    pub recorded_ms_30d: i64,
}

const USAGE_WINDOW: Ms = 30 * 24 * 3_600_000;

/// Per project: the latest span start and the time its spans recorded in the window before `now`.
async fn usage_by_project(db: &Db, now: Ms) -> AppResult<HashMap<String, (Option<Ms>, i64)>> {
    let rows: Vec<(String, Option<Ms>, i64)> = sqlx::query_as(
        r#"SELECT r.project_id, max(s.start),
             coalesce(sum(max(0, min(coalesce(s."end", ?1), ?1) - max(s.start, ?2))), 0)
           FROM recordings r JOIN recording_spans s ON s.recording_id = r.id GROUP BY r.project_id"#,
    )
    .bind(now)
    .bind(now - USAGE_WINDOW)
    .fetch_all(db.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, last_used, recorded)| (id, (last_used, recorded)))
        .collect())
}

pub async fn list_items(
    db: &Db,
    include_archived: bool,
    now: Ms,
) -> AppResult<Vec<ProjectListItem>> {
    let clients: HashMap<String, Client> = clients::list(db, true)
        .await?
        .into_iter()
        .map(|c| (c.id.clone(), c))
        .collect();
    let usage = usage_by_project(db, now).await?;
    Ok(list(db, include_archived)
        .await?
        .into_iter()
        .map(|project| {
            let (last_used, recorded_ms_30d) = usage.get(&project.id).copied().unwrap_or_default();
            ProjectListItem {
                client: project
                    .client_id
                    .as_ref()
                    .and_then(|id| clients.get(id))
                    .cloned(),
                project,
                last_used,
                recorded_ms_30d,
            }
        })
        .collect())
}

pub async fn count(db: &Db) -> AppResult<i64> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM projects")
        .fetch_one(db.pool())
        .await?)
}

pub(super) async fn all(conn: &mut SqliteConnection) -> AppResult<Vec<Project>> {
    Ok(
        sqlx::query_as("SELECT * FROM projects ORDER BY created_at, id")
            .fetch_all(conn)
            .await?,
    )
}

fn found(id: &str, project: Option<Project>) -> AppResult<Project> {
    project.ok_or_else(|| AppError::NotFound(format!("No project {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;
    use crate::test_support::{client, client_with_color};

    fn input(name: &str) -> ProjectInput {
        ProjectInput {
            client_id: None,
            name: name.into(),
            color: "blue".into(),
            shade: None,
            billable: false,
        }
    }

    fn shades(projects: &[Project]) -> Vec<u8> {
        projects.iter().map(|p| p.shade).collect()
    }

    fn names(projects: Vec<Project>) -> Vec<String> {
        projects.into_iter().map(|p| p.name).collect()
    }

    #[tokio::test]
    async fn insert_stores_every_field() {
        let db = Db::open_memory().await.unwrap();
        let client = client_with_color(&db, "Acme", "teal").await;
        let project = insert(
            &db,
            ProjectInput {
                client_id: Some(client.id.clone()),
                name: "  Website ".into(),
                color: "red".into(),
                shade: Some(6),
                billable: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(project.client_id, Some(client.id));
        assert_eq!(project.name, "Website");
        assert_eq!(project.color, "teal");
        assert_eq!(project.shade, 6);
        assert_eq!(project.archived_at, None);
        assert_eq!(get(&db, &project.id).await.unwrap(), project);
        assert_eq!(count(&db).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn a_project_is_not_billable_unless_marked() {
        let db = Db::open_memory().await.unwrap();

        let project = insert(&db, input("P")).await.unwrap();

        assert!(!project.billable);
    }

    #[tokio::test]
    async fn the_billable_flag_is_stored_and_read_back() {
        let db = Db::open_memory().await.unwrap();

        let project = insert(
            &db,
            ProjectInput {
                billable: true,
                ..input("P")
            },
        )
        .await
        .unwrap();

        assert!(project.billable);
        assert!(get(&db, &project.id).await.unwrap().billable);
        assert!(list(&db, false).await.unwrap()[0].billable);
    }

    #[tokio::test]
    async fn the_billable_flag_switches_both_ways_on_update() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();

        for billable in [true, false] {
            let updated = update(
                &db,
                &project.id,
                ProjectInput {
                    billable,
                    ..input("P")
                },
            )
            .await
            .unwrap();

            assert_eq!(updated.billable, billable);
            assert_eq!(get(&db, &project.id).await.unwrap().billable, billable);
        }
    }

    #[tokio::test]
    async fn an_archived_project_keeps_its_billable_flag() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(
            &db,
            ProjectInput {
                billable: true,
                ..input("P")
            },
        )
        .await
        .unwrap();

        let archived = set_archived(&db, &project.id, true).await.unwrap();
        let restored = set_archived(&db, &project.id, false).await.unwrap();

        assert!(archived.billable);
        assert!(restored.billable);
    }

    #[tokio::test]
    async fn invalid_input_is_rejected() {
        let db = Db::open_memory().await.unwrap();
        let cases = [
            (
                "blank name",
                ProjectInput {
                    name: "  ".into(),
                    ..input("x")
                },
            ),
            (
                "unknown color",
                ProjectInput {
                    color: "chartreuse".into(),
                    ..input("x")
                },
            ),
        ];
        for (case, bad) in cases {
            let err = insert(&db, bad).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{case}");
        }
        assert_eq!(count(&db).await.unwrap(), 0);
    }

    fn field_of(err: AppError) -> String {
        match err {
            AppError::InvalidField { field, .. } => field,
            other => panic!("expected an invalid field, got {other:?}"),
        }
    }

    async fn in_client(db: &Db, client_id: &str, name: &str) -> Project {
        insert(
            db,
            ProjectInput {
                client_id: Some(client_id.into()),
                ..input(name)
            },
        )
        .await
        .unwrap()
    }

    async fn record(db: &Db, project_id: &str, start: Ms, end: Option<Ms>) {
        crate::test_support::recording(db, project_id, start, end).await;
    }

    #[tokio::test]
    async fn invalid_input_names_the_offending_field() {
        let db = Db::open_memory().await.unwrap();
        let cases = [
            (
                "blank name",
                ProjectInput {
                    name: "  ".into(),
                    ..input("x")
                },
                "name",
            ),
            (
                "name over the limit",
                ProjectInput {
                    name: "n".repeat(crate::db::MAX_NAME_CHARS + 1),
                    ..input("x")
                },
                "name",
            ),
            (
                "unknown color",
                ProjectInput {
                    color: "chartreuse".into(),
                    ..input("x")
                },
                "color",
            ),
        ];
        for (case, bad, field) in cases {
            let err = insert(&db, bad).await.unwrap_err();
            assert_eq!(field_of(err), field, "{case}");
        }
    }

    #[tokio::test]
    async fn an_empty_color_becomes_the_default() {
        let db = Db::open_memory().await.unwrap();
        for color in ["", "  "] {
            let project = insert(
                &db,
                ProjectInput {
                    color: color.into(),
                    ..input(&format!("P{}", color.len()))
                },
            )
            .await
            .unwrap();
            assert_eq!(project.color, DEFAULT_COLOR, "{color:?}");
        }
        assert_eq!(DEFAULT_COLOR, "orange");
    }

    #[tokio::test]
    async fn an_archived_client_is_refused_on_create_and_when_switching_to_it() {
        let db = Db::open_memory().await.unwrap();
        let old = client(&db, "Old").await;
        clients::set_archived(&db, &old.id, true).await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();

        let err = insert(
            &db,
            ProjectInput {
                client_id: Some(old.id.clone()),
                ..input("Q")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(field_of(err), "clientId");

        let err = update(
            &db,
            &project.id,
            ProjectInput {
                client_id: Some(old.id),
                ..input("P")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(field_of(err), "clientId");
    }

    #[tokio::test]
    async fn a_project_can_be_edited_while_it_keeps_its_archived_client() {
        let db = Db::open_memory().await.unwrap();
        let client = client(&db, "Acme").await;
        let project = in_client(&db, &client.id, "P").await;
        clients::set_archived(&db, &client.id, true).await.unwrap();
        set_archived(&db, &project.id, false).await.unwrap();

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                client_id: Some(client.id.clone()),
                name: "Renamed".into(),
                ..input("x")
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.client_id, Some(client.id));
    }

    #[tokio::test]
    async fn names_are_unique_per_client_ignoring_case_and_archive_state() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let globex = client(&db, "Globex").await;
        insert(&db, input("Äpfel")).await.unwrap();
        let archived = in_client(&db, &acme.id, "Web").await;
        set_archived(&db, &archived.id, true).await.unwrap();

        for (client_id, name) in [
            (None, "Äpfel"),
            (None, "äpfel"),
            (None, " ÄPFEL "),
            (Some(acme.id.clone()), "web"),
            (Some(acme.id.clone()), "WEB"),
        ] {
            let err = insert(
                &db,
                ProjectInput {
                    client_id: client_id.clone(),
                    ..input(name)
                },
            )
            .await
            .unwrap_err();
            assert_eq!(field_of(err), "name", "{client_id:?} {name:?}");
        }

        in_client(&db, &globex.id, "Web").await;
        insert(&db, input("Web")).await.unwrap();
    }

    #[tokio::test]
    async fn an_update_keeps_its_own_name_but_cannot_take_a_siblings_or_move_into_a_clash() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let first = insert(&db, input("First")).await.unwrap();
        insert(&db, input("Second")).await.unwrap();
        in_client(&db, &acme.id, "First").await;

        let same = update(&db, &first.id, input("first")).await.unwrap();
        assert_eq!(same.name, "first");

        let err = update(&db, &first.id, input("SECOND")).await.unwrap_err();
        assert_eq!(field_of(err), "name");

        let err = update(
            &db,
            &first.id,
            ProjectInput {
                client_id: Some(acme.id),
                ..input("First")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(field_of(err), "name");
    }

    #[tokio::test]
    async fn editing_an_archived_project_leaves_it_archived() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();
        let archived_at = set_archived(&db, &project.id, true)
            .await
            .unwrap()
            .archived_at;

        let updated = update(&db, &project.id, input("Renamed")).await.unwrap();

        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.archived_at, archived_at);
        assert!(archived_at.is_some());
    }

    #[tokio::test]
    async fn unknown_client_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        let err = insert(
            &db,
            ProjectInput {
                client_id: Some("nope".into()),
                ..input("x")
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn unknown_project_is_not_found() {
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
    async fn update_replaces_all_fields() {
        let db = Db::open_memory().await.unwrap();
        let client = client_with_color(&db, "Acme", "red").await;
        let project = insert(&db, input("Old")).await.unwrap();

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                client_id: Some(client.id.clone()),
                name: "New".into(),
                color: "blue".into(),
                shade: Some(2),
                billable: false,
            },
        )
        .await
        .unwrap();

        assert_eq!(
            updated,
            Project {
                client_id: Some(client.id),
                name: "New".into(),
                color: "red".into(),
                shade: 2,
                ..project
            }
        );
    }

    #[tokio::test]
    async fn archiving_keeps_the_first_timestamp_and_unarchiving_clears_it() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();

        let first_at = set_archived(&db, &project.id, true)
            .await
            .unwrap()
            .archived_at
            .expect("archived");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        let again = set_archived(&db, &project.id, true).await.unwrap();
        assert_eq!(again.archived_at, Some(first_at));
        let restored = set_archived(&db, &project.id, false).await.unwrap();
        assert_eq!(restored.archived_at, None);
    }

    #[tokio::test]
    async fn list_hides_archived_projects_unless_asked_and_sorts_ignoring_case() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, input("banana")).await.unwrap();
        insert(&db, input("Cherry")).await.unwrap();
        insert(&db, input("apple")).await.unwrap();
        let old = insert(&db, input("Berry")).await.unwrap();
        set_archived(&db, &old.id, true).await.unwrap();

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
    async fn resolve_finds_a_project_by_id_even_when_archived() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();
        assert_eq!(resolve(&db, &project.id).await.unwrap(), project);

        let archived = set_archived(&db, &project.id, true).await.unwrap();
        assert_eq!(resolve(&db, &project.id).await.unwrap(), archived);
    }

    #[tokio::test]
    async fn resolve_matches_a_name_ignoring_case_and_a_unique_prefix() {
        let db = Db::open_memory().await.unwrap();
        for name in ["Ärger", "Website", "Other"] {
            insert(&db, input(name)).await.unwrap();
        }

        for (text, expected) in [
            ("ärger", "Ärger"),
            ("ÄRGER", "Ärger"),
            ("  website ", "Website"),
            ("är", "Ärger"),
            ("ÄR", "Ärger"),
            ("web", "Website"),
            ("o", "Other"),
        ] {
            assert_eq!(resolve(&db, text).await.unwrap().name, expected, "{text:?}");
        }
    }

    #[tokio::test]
    async fn resolve_prefers_an_exact_name_over_longer_prefix_matches() {
        let db = Db::open_memory().await.unwrap();
        let web = insert(&db, input("Web")).await.unwrap();
        insert(&db, input("Website")).await.unwrap();

        assert_eq!(resolve(&db, "web").await.unwrap(), web);
    }

    #[tokio::test]
    async fn resolve_without_a_match_is_not_found() {
        let db = Db::open_memory().await.unwrap();
        insert(&db, input("Website")).await.unwrap();
        let gone = insert(&db, input("Archive")).await.unwrap();
        set_archived(&db, &gone.id, true).await.unwrap();

        for text in ["zzz", "archive", "arch", "", "   "] {
            let err = resolve(&db, text).await.unwrap_err();
            assert_eq!(err.kind(), ErrorKind::NotFound, "{text:?}");
        }
    }

    #[tokio::test]
    async fn resolve_with_several_matches_lists_the_candidates() {
        let db = Db::open_memory().await.unwrap();
        let alpha = insert(&db, input("Alpha")).await.unwrap();
        let alps = insert(&db, input("Alps")).await.unwrap();
        insert(&db, input("Beta")).await.unwrap();

        let err = resolve(&db, "al").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(
            err.to_string(),
            format!(
                "`al` matches several projects: Alpha ({}), Alps ({})",
                alpha.id, alps.id
            )
        );
    }

    #[tokio::test]
    async fn the_same_name_under_two_clients_is_ambiguous() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let globex = client(&db, "Globex").await;
        in_client(&db, &acme.id, "Web").await;
        in_client(&db, &globex.id, "Web").await;

        let err = resolve(&db, "web").await.unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn recent_lists_active_recorded_projects_by_latest_recording() {
        let db = Db::open_memory().await.unwrap();
        let first = insert(&db, input("First")).await.unwrap();
        let second = insert(&db, input("Second")).await.unwrap();
        insert(&db, input("Never")).await.unwrap();
        let archived = insert(&db, input("Archived")).await.unwrap();
        record(&db, &first.id, 1_000, Some(2_000)).await;
        record(&db, &second.id, 3_000, Some(4_000)).await;
        record(&db, &first.id, 5_000, Some(6_000)).await;
        record(&db, &archived.id, 7_000, Some(8_000)).await;
        set_archived(&db, &archived.id, true).await.unwrap();

        assert_eq!(names(recent(&db, 10).await.unwrap()), ["First", "Second"]);
        assert_eq!(names(recent(&db, 1).await.unwrap()), ["First"]);
        assert!(recent(&db, 0).await.unwrap().is_empty());
    }

    const DAY: Ms = 24 * 3_600_000;
    const HOUR: Ms = 3_600_000;

    async fn record_spans(db: &Db, project_id: &str, spans: &[(Ms, Option<Ms>)]) {
        crate::db::recordings::insert(
            db,
            crate::db::recordings::NewRecording {
                id: uuid::Uuid::new_v4().to_string(),
                project_id: project_id.into(),
                spans: spans.to_vec(),
                note: String::new(),
                tag_id: None,
            },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn recent_orders_by_the_latest_span_of_a_recording() {
        let db = Db::open_memory().await.unwrap();
        let spread = insert(&db, input("Spread")).await.unwrap();
        let single = insert(&db, input("Single")).await.unwrap();
        record_spans(
            &db,
            &spread.id,
            &[(1_000, Some(2_000)), (9_000, Some(9_500))],
        )
        .await;
        record_spans(&db, &single.id, &[(5_000, Some(6_000))]).await;

        assert_eq!(names(recent(&db, 10).await.unwrap()), ["Spread", "Single"]);
    }

    #[tokio::test]
    async fn list_items_count_every_span_of_a_recording_in_the_last_30_days() {
        let db = Db::open_memory().await.unwrap();
        let now = 100 * DAY;
        let window_start = now - 30 * DAY;
        let split = insert(&db, input("Split")).await.unwrap();
        let timer = insert(&db, input("Timer")).await.unwrap();
        record_spans(
            &db,
            &split.id,
            &[
                (window_start - HOUR, Some(window_start + HOUR)),
                (75 * DAY, Some(75 * DAY + 2 * HOUR)),
                (90 * DAY, Some(90 * DAY + HOUR / 2)),
            ],
        )
        .await;
        record_spans(
            &db,
            &timer.id,
            &[(20 * DAY, Some(20 * DAY + HOUR)), (now - HOUR, None)],
        )
        .await;

        let items = list_items(&db, false, now).await.unwrap();

        let summary: Vec<_> = items
            .iter()
            .map(|i| (i.project.id.as_str(), i.last_used, i.recorded_ms_30d))
            .collect();
        assert_eq!(
            summary,
            [
                (split.id.as_str(), Some(90 * DAY), 3 * HOUR + HOUR / 2),
                (timer.id.as_str(), Some(now - HOUR), HOUR),
            ]
        );
    }

    #[tokio::test]
    async fn list_items_attach_clients_last_use_and_the_last_30_days() {
        let db = Db::open_memory().await.unwrap();
        let now = 100 * DAY;
        let window_start = now - 30 * DAY;
        let acme = client(&db, "Acme").await;
        let busy = in_client(&db, &acme.id, "Busy").await;
        let old = insert(&db, input("Old")).await.unwrap();
        let running = insert(&db, input("Running")).await.unwrap();
        let unused = insert(&db, input("Unused")).await.unwrap();

        record(&db, &old.id, 10 * DAY, Some(10 * DAY + HOUR)).await;
        record(
            &db,
            &busy.id,
            window_start - HOUR,
            Some(window_start + 2 * HOUR),
        )
        .await;
        record(&db, &busy.id, 80 * DAY, Some(80 * DAY + HOUR / 2)).await;
        record(&db, &running.id, now - HOUR, None).await;

        let items = list_items(&db, false, now).await.unwrap();

        let summary: Vec<_> = items
            .iter()
            .map(|i| {
                (
                    i.project.id.as_str(),
                    i.client.as_ref().map(|c| c.id.as_str()),
                    i.last_used,
                    i.recorded_ms_30d,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    busy.id.as_str(),
                    Some(acme.id.as_str()),
                    Some(80 * DAY),
                    2 * HOUR + HOUR / 2
                ),
                (old.id.as_str(), None, Some(10 * DAY), 0),
                (running.id.as_str(), None, Some(now - HOUR), HOUR),
                (unused.id.as_str(), None, None, 0),
            ]
        );
    }

    #[tokio::test]
    async fn list_items_show_archived_projects_and_clients_only_when_asked() {
        let db = Db::open_memory().await.unwrap();
        let client = client(&db, "Acme").await;
        let project = in_client(&db, &client.id, "P").await;
        insert(&db, input("Active")).await.unwrap();
        let archived_client = clients::set_archived(&db, &client.id, true).await.unwrap();

        let active = list_items(&db, false, DAY).await.unwrap();
        let active_names: Vec<_> = active.iter().map(|i| i.project.name.as_str()).collect();
        assert_eq!(active_names, ["Active"]);

        let all = list_items(&db, true, DAY).await.unwrap();
        assert_eq!(all.len(), 2);
        let archived = all.iter().find(|i| i.project.id == project.id).unwrap();
        assert_eq!(archived.client, Some(archived_client));
        assert!(archived.project.archived_at.is_some());
    }

    fn with_client(client: &Client, name: &str) -> ProjectInput {
        ProjectInput {
            client_id: Some(client.id.clone()),
            ..input(name)
        }
    }

    #[tokio::test]
    async fn a_project_with_a_client_takes_the_clients_color_whatever_it_is_given() {
        let db = Db::open_memory().await.unwrap();
        let client = client_with_color(&db, "Acme", "teal").await;

        for (i, color) in ["red", "", "chartreuse"].into_iter().enumerate() {
            let created = insert(
                &db,
                ProjectInput {
                    color: color.into(),
                    ..with_client(&client, &format!("P{i}"))
                },
            )
            .await
            .unwrap();
            assert_eq!(created.color, "teal", "insert {color:?}");

            let loose = insert(&db, input(&format!("Loose{i}"))).await.unwrap();
            let moved = update(
                &db,
                &loose.id,
                ProjectInput {
                    color: color.into(),
                    ..with_client(&client, &format!("Loose{i}"))
                },
            )
            .await
            .unwrap();
            assert_eq!(moved.color, "teal", "update {color:?}");
        }
    }

    #[tokio::test]
    async fn a_client_less_project_keeps_the_color_it_is_given() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(
            &db,
            ProjectInput {
                color: "purple".into(),
                ..input("P")
            },
        )
        .await
        .unwrap();
        assert_eq!(project.color, "purple");

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                color: "gold".into(),
                ..input("P")
            },
        )
        .await
        .unwrap();
        assert_eq!(updated.color, "gold");
    }

    #[tokio::test]
    async fn moving_a_project_out_of_its_client_gives_it_the_color_it_is_sent() {
        let db = Db::open_memory().await.unwrap();
        let client = client_with_color(&db, "Acme", "teal").await;
        let project = insert(&db, with_client(&client, "P")).await.unwrap();

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                color: "red".into(),
                ..input("P")
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.color, "red");
        assert_eq!(updated.client_id, None);
    }

    #[tokio::test]
    async fn a_given_shade_is_stored_and_one_past_the_last_is_refused() {
        let db = Db::open_memory().await.unwrap();
        for shade in 0..SHADES {
            let project = insert(
                &db,
                ProjectInput {
                    shade: Some(shade),
                    ..input(&format!("P{shade}"))
                },
            )
            .await
            .unwrap();
            assert_eq!(project.shade, shade);
        }

        let existing = insert(&db, input("Existing")).await.unwrap();
        for shade in [SHADES, 10, 255] {
            let bad = ProjectInput {
                shade: Some(shade),
                ..input("Bad")
            };
            for err in [
                insert(&db, bad.clone()).await.unwrap_err(),
                update(&db, &existing.id, bad).await.unwrap_err(),
            ] {
                match err {
                    AppError::InvalidField { field, message } => {
                        assert_eq!(field, "shade", "{shade}");
                        assert_eq!(message, "must be between 0 and 8", "{shade}");
                    }
                    other => panic!("expected an invalid field, got {other:?}"),
                }
            }
        }
        assert_eq!(count(&db).await.unwrap(), 10);
    }

    #[tokio::test]
    async fn new_projects_of_a_client_get_the_shades_in_shade_order() {
        let db = Db::open_memory().await.unwrap();
        let client = client(&db, "Acme").await;
        let mut created = Vec::new();
        for i in 0..SHADES {
            let input = with_client(&client, &format!("P{i}"));
            created.push(insert(&db, input).await.unwrap());
        }
        assert_eq!(shades(&created), SHADE_ORDER);
    }

    #[tokio::test]
    async fn when_all_shades_are_taken_the_order_starts_over_by_sibling_count() {
        let db = Db::open_memory().await.unwrap();
        let client = client(&db, "Acme").await;
        let mut created = Vec::new();
        for i in 0..SHADES as usize + 3 {
            let input = with_client(&client, &format!("P{i}"));
            created.push(insert(&db, input).await.unwrap());
        }
        assert_eq!(
            shades(&created[SHADES as usize..]),
            [SHADE_ORDER[0], SHADE_ORDER[1], SHADE_ORDER[2]]
        );
    }

    #[tokio::test]
    async fn a_new_project_takes_the_first_free_shade_counting_archived_siblings() {
        let db = Db::open_memory().await.unwrap();
        let client = client(&db, "Acme").await;
        let explicit = insert(
            &db,
            ProjectInput {
                shade: Some(4),
                ..with_client(&client, "First")
            },
        )
        .await
        .unwrap();
        set_archived(&db, &explicit.id, true).await.unwrap();

        let next = insert(&db, with_client(&client, "Second")).await.unwrap();
        let after = insert(&db, with_client(&client, "Third")).await.unwrap();

        assert_eq!((next.shade, after.shade), (1, 7));
    }

    #[tokio::test]
    async fn shades_are_handed_out_per_client_and_client_less_projects_share_one_pool() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let globex = client(&db, "Globex").await;

        let a1 = insert(&db, with_client(&acme, "A1")).await.unwrap();
        let a2 = insert(&db, with_client(&acme, "A2")).await.unwrap();
        let g1 = insert(&db, with_client(&globex, "G1")).await.unwrap();
        let l1 = insert(&db, input("L1")).await.unwrap();
        let l2 = insert(&db, input("L2")).await.unwrap();

        let [first, second] = [SHADE_ORDER[0], SHADE_ORDER[1]];
        assert_eq!(
            shades(&[a1, a2, g1, l1, l2]),
            [first, second, first, first, second]
        );
    }

    #[tokio::test]
    async fn an_update_without_a_shade_keeps_it_within_the_same_client() {
        let db = Db::open_memory().await.unwrap();
        let client = client(&db, "Acme").await;
        insert(&db, with_client(&client, "First")).await.unwrap();
        let second = insert(&db, with_client(&client, "Second")).await.unwrap();
        let loose = insert(&db, input("Loose")).await.unwrap();

        let renamed = update(&db, &second.id, with_client(&client, "Renamed"))
            .await
            .unwrap();
        let loose_renamed = update(&db, &loose.id, input("Loose 2")).await.unwrap();

        assert_eq!(renamed.shade, second.shade);
        assert_eq!(loose_renamed.shade, loose.shade);
    }

    #[tokio::test]
    async fn an_update_with_a_shade_replaces_it() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();

        let updated = update(
            &db,
            &project.id,
            ProjectInput {
                shade: Some(0),
                ..input("P")
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.shade, 0);
    }

    #[tokio::test]
    async fn changing_the_client_without_a_shade_takes_the_next_free_shade_of_the_new_client() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let globex = client(&db, "Globex").await;
        insert(&db, with_client(&globex, "G1")).await.unwrap();
        insert(&db, with_client(&globex, "G2")).await.unwrap();
        let mover = insert(&db, with_client(&acme, "Mover")).await.unwrap();
        assert_eq!(mover.shade, SHADE_ORDER[0]);

        let moved = update(&db, &mover.id, with_client(&globex, "Mover"))
            .await
            .unwrap();
        assert_eq!(moved.shade, SHADE_ORDER[2]);

        let loose = update(&db, &mover.id, input("Mover")).await.unwrap();
        assert_eq!(loose.shade, SHADE_ORDER[0]);
    }

    #[tokio::test]
    async fn changing_the_client_while_naming_a_shade_uses_that_shade() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let project = insert(&db, input("P")).await.unwrap();

        let moved = update(
            &db,
            &project.id,
            ProjectInput {
                shade: Some(8),
                ..with_client(&acme, "P")
            },
        )
        .await
        .unwrap();

        assert_eq!(moved.shade, 8);
    }

    #[tokio::test]
    async fn a_list_item_serializes_with_camel_case_fields() {
        let db = Db::open_memory().await.unwrap();
        let project = insert(&db, input("P")).await.unwrap();
        record(&db, &project.id, 2 * HOUR, Some(3 * HOUR)).await;

        let items = list_items(&db, false, 4 * HOUR).await.unwrap();

        assert_eq!(
            serde_json::to_value(&items[0]).unwrap(),
            serde_json::json!({
                "project": {
                    "id": project.id,
                    "clientId": null,
                    "name": "P",
                    "color": "blue",
                    "shade": 4,
                    "billable": false,
                    "archivedAt": null,
                    "createdAt": project.created_at,
                },
                "client": null,
                "lastUsed": 2 * HOUR,
                "recordedMs30d": HOUR,
            })
        );
    }
}
