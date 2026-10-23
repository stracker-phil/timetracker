//! `timetracker projects`, `tags` and `clients`: the names a script passes back, with their ids.

use serde::{Deserialize, Serialize};

use crate::db::{Db, clients, projects, tags};
use crate::error::AppResult;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectEntry {
    pub id: String,
    pub name: String,
    pub client_id: Option<String>,
    pub client: Option<String>,
    pub billable: bool,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientEntry {
    pub id: String,
    pub name: String,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEntry {
    pub id: String,
    pub name: String,
    pub emoji: Option<String>,
}

/// By name; archived ones only with `include_archived`.
pub async fn projects(db: &Db, include_archived: bool) -> AppResult<Vec<ProjectEntry>> {
    let clients = clients::list(db, true).await?;
    Ok(projects::list(db, include_archived)
        .await?
        .into_iter()
        .map(|project| ProjectEntry {
            client: project
                .client_id
                .as_ref()
                .and_then(|id| clients.iter().find(|c| &c.id == id))
                .map(|c| c.name.clone()),
            id: project.id,
            name: project.name,
            client_id: project.client_id,
            billable: project.billable,
            archived: project.archived_at.is_some(),
        })
        .collect())
}

/// By name; archived ones only with `include_archived`.
pub async fn clients(db: &Db, include_archived: bool) -> AppResult<Vec<ClientEntry>> {
    Ok(clients::list(db, include_archived)
        .await?
        .into_iter()
        .map(|client| ClientEntry {
            id: client.id,
            name: client.name,
            archived: client.archived_at.is_some(),
        })
        .collect())
}

/// By name.
pub async fn tags(db: &Db) -> AppResult<Vec<TagEntry>> {
    Ok(tags::list(db)
        .await?
        .into_iter()
        .map(|tag| TagEntry {
            id: tag.id,
            name: tag.name,
            emoji: tag.emoji,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db::projects::{Project, ProjectInput};
    use crate::test_support::{client, project};

    async fn billable_project(db: &Db, name: &str, client_id: &str) -> Project {
        projects::insert(
            db,
            ProjectInput {
                client_id: Some(client_id.into()),
                name: name.into(),
                color: "blue".into(),
                shade: None,
                billable: true,
            },
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn projects_are_listed_by_name_with_their_client_and_billable_flag() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        let portal = billable_project(&db, "Portal", &acme.id).await;
        let admin = project(&db, "admin").await;

        assert_eq!(
            projects(&db, false).await.unwrap(),
            [
                ProjectEntry {
                    id: admin.id,
                    name: "admin".into(),
                    client_id: None,
                    client: None,
                    billable: false,
                    archived: false,
                },
                ProjectEntry {
                    id: portal.id,
                    name: "Portal".into(),
                    client_id: Some(acme.id),
                    client: Some("Acme".into()),
                    billable: true,
                    archived: false,
                },
            ]
        );
    }

    #[tokio::test]
    async fn archived_projects_are_listed_only_on_request() {
        let db = Db::open_memory().await.unwrap();
        let kept = project(&db, "Kept").await;
        let old = project(&db, "Old").await;
        projects::set_archived(&db, &old.id, true).await.unwrap();

        let active = projects(&db, false).await.unwrap();
        let all = projects(&db, true).await.unwrap();

        assert_eq!(
            active
                .iter()
                .map(|p| (&p.id, p.archived))
                .collect::<Vec<_>>(),
            [(&kept.id, false)]
        );
        assert_eq!(
            all.iter().map(|p| (&p.id, p.archived)).collect::<Vec<_>>(),
            [(&kept.id, false), (&old.id, true)]
        );
    }

    #[tokio::test]
    async fn the_projects_of_an_archived_client_keep_naming_it() {
        let db = Db::open_memory().await.unwrap();
        let acme = client(&db, "Acme").await;
        billable_project(&db, "Portal", &acme.id).await;
        clients::set_archived(&db, &acme.id, true).await.unwrap();

        assert!(projects(&db, false).await.unwrap().is_empty());
        let all = projects(&db, true).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(
            (all[0].client.as_deref(), all[0].archived),
            (Some("Acme"), true)
        );
    }

    #[tokio::test]
    async fn clients_are_listed_by_name_and_archived_ones_only_on_request() {
        let db = Db::open_memory().await.unwrap();
        let globex = client(&db, "Globex").await;
        let acme = client(&db, "Acme").await;
        clients::set_archived(&db, &globex.id, true).await.unwrap();

        assert_eq!(
            clients(&db, false).await.unwrap(),
            [ClientEntry {
                id: acme.id.clone(),
                name: "Acme".into(),
                archived: false,
            }]
        );
        assert_eq!(
            clients(&db, true).await.unwrap(),
            [
                ClientEntry {
                    id: acme.id,
                    name: "Acme".into(),
                    archived: false,
                },
                ClientEntry {
                    id: globex.id,
                    name: "Globex".into(),
                    archived: true,
                },
            ]
        );
    }

    #[tokio::test]
    async fn tags_are_listed_by_name_with_their_emoji() {
        let db = Db::open_memory().await.unwrap();
        let focus = tags::insert(&db, "focus").await.unwrap();
        let focus = tags::set_emoji(&db, &focus.id, Some("🎯")).await.unwrap();
        let plain = tags::insert(&db, "plain").await.unwrap();

        let listed = tags(&db).await.unwrap();

        assert!(listed.contains(&TagEntry {
            id: focus.id,
            name: "focus".into(),
            emoji: Some("🎯".into()),
        }));
        assert!(listed.contains(&TagEntry {
            id: plain.id,
            name: "plain".into(),
            emoji: None,
        }));
        let names: Vec<String> = listed.iter().map(|t| t.name.to_lowercase()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn entries_have_camel_case_fields() {
        assert_eq!(
            serde_json::to_value(ProjectEntry {
                id: "p1".into(),
                name: "Portal".into(),
                client_id: Some("c1".into()),
                client: Some("Acme".into()),
                billable: true,
                archived: false,
            })
            .unwrap(),
            json!({
                "id": "p1", "name": "Portal", "clientId": "c1", "client": "Acme",
                "billable": true, "archived": false
            })
        );
        assert_eq!(
            serde_json::to_value(ClientEntry {
                id: "c1".into(),
                name: "Acme".into(),
                archived: true,
            })
            .unwrap(),
            json!({ "id": "c1", "name": "Acme", "archived": true })
        );
        assert_eq!(
            serde_json::to_value(TagEntry {
                id: "t1".into(),
                name: "focus".into(),
                emoji: None,
            })
            .unwrap(),
            json!({ "id": "t1", "name": "focus", "emoji": null })
        );
    }
}
