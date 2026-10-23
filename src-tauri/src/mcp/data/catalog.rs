//! Projects, clients and tags.

use serde::Serialize;

use super::Core;
use crate::db::{clients, projects, tags};
use crate::error::AppResult;

#[derive(Serialize)]
pub struct ProjectOut {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client: Option<String>,
    pub billable: bool,
    pub archived: bool,
}

#[derive(Serialize)]
pub struct ClientOut {
    pub id: String,
    pub name: String,
    pub archived: bool,
}

#[derive(Serialize)]
pub struct TagOut {
    pub id: String,
    pub name: String,
}

impl Core {
    pub async fn projects(&self) -> AppResult<Vec<ProjectOut>> {
        let db = self.db().await?;
        let include_archived = true;
        let clients = clients::list(db, include_archived).await?;
        Ok(projects::list(db, include_archived)
            .await?
            .into_iter()
            .map(|project| ProjectOut {
                client: project
                    .client_id
                    .as_ref()
                    .and_then(|id| clients.iter().find(|c| &c.id == id))
                    .map(|c| c.name.clone()),
                id: project.id,
                name: project.name,
                color: project.color,
                billable: project.billable,
                archived: project.archived_at.is_some(),
            })
            .collect())
    }

    pub async fn clients(&self) -> AppResult<Vec<ClientOut>> {
        Ok(clients::list(self.db().await?, true)
            .await?
            .into_iter()
            .map(|client| ClientOut {
                archived: client.archived_at.is_some(),
                id: client.id,
                name: client.name,
            })
            .collect())
    }

    pub async fn tags(&self) -> AppResult<Vec<TagOut>> {
        Ok(tags::list(self.db().await?)
            .await?
            .into_iter()
            .map(|tag| TagOut {
                id: tag.id,
                name: tag.name,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::db;
    use crate::mcp::data::fixtures::{named, seeded};

    #[tokio::test]
    async fn projects_list_their_client_color_billable_and_archived_flag() {
        let seed = seeded().await;
        projects::set_archived(&seed.db, &seed.podcast.id, true)
            .await
            .unwrap();

        let out = serde_json::to_value(seed.core.projects().await.unwrap()).unwrap();

        assert_eq!(out.as_array().unwrap().len(), 2);
        assert_eq!(
            named(&out, "name", "Portal"),
            &json!({
                "id": seed.portal.id,
                "name": "Portal",
                "color": seed.portal.color,
                "client": "Acme",
                "billable": true,
                "archived": false,
            })
        );
        assert_eq!(
            named(&out, "name", "Podcast"),
            &json!({
                "id": seed.podcast.id,
                "name": "Podcast",
                "color": seed.podcast.color,
                "billable": false,
                "archived": true,
            })
        );
    }

    #[tokio::test]
    async fn clients_list_archived_ones_too_with_the_flag() {
        let seed = seeded().await;
        let globex = crate::test_support::client(&seed.db, "Globex").await;
        clients::set_archived(&seed.db, &globex.id, true)
            .await
            .unwrap();

        let out = serde_json::to_value(seed.core.clients().await.unwrap()).unwrap();

        assert_eq!(out.as_array().unwrap().len(), 2);
        assert_eq!(
            named(&out, "name", "Globex"),
            &json!({"id": globex.id, "name": "Globex", "archived": true})
        );
        assert_eq!(named(&out, "name", "Acme")["archived"], false);
    }

    #[tokio::test]
    async fn tags_list_every_tag_with_its_id() {
        let seed = seeded().await;

        let out = serde_json::to_value(seed.core.tags().await.unwrap()).unwrap();

        let mut names: Vec<&str> = out
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap())
            .collect();
        names.sort();
        let mut expected = db::tags::DEFAULT_TAGS.to_vec();
        expected.sort();
        assert_eq!(names, expected);
        assert_eq!(
            named(&out, "name", "development"),
            &json!({"id": seed.development.id, "name": "development"})
        );
    }
}
