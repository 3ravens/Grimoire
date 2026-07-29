
//! Declarative starter vault layouts for the installation wizard.

use sqlx::{Sqlite, Transaction};

use crate::note_store::EncryptedNoteStore;
use crate::{AppError, AppResult};

/// Starter pack identifiers (persisted in `wizard_starter_pack_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarterPackId {
    Empty,
    Pkm,
    BulletJournal,
    Para,
}

impl StarterPackId {
    pub fn as_str(self) -> &'static str {
        match self {
            StarterPackId::Empty => "empty",
            StarterPackId::Pkm => "pkm",
            StarterPackId::BulletJournal => "bullet_journal",
            StarterPackId::Para => "para",
        }
    }
}

pub fn parse_pack_id(raw: &str) -> AppResult<StarterPackId> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "empty" | "none" => Ok(StarterPackId::Empty),
        "pkm" | "knowledge" | "knowledge_builders" => Ok(StarterPackId::Pkm),
        "bullet_journal" | "bujo" | "bullet" => Ok(StarterPackId::BulletJournal),
        "para" => Ok(StarterPackId::Para),
        other => Err(AppError::InvalidInput(format!("Unknown starter pack id: {other}"))),
    }
}

/// Plaintext tuples for FTS + Lance indexing after the transaction commits.
pub async fn apply_starter_pack_tx(
    store: &EncryptedNoteStore<'_>,
    tx: &mut Transaction<'_, Sqlite>,
    pack: StarterPackId,
) -> AppResult<Vec<(i64, String, String)>> {
    let mut indexed: Vec<(i64, String, String)> = Vec::new();

    match pack {
        StarterPackId::Empty => {}
        StarterPackId::Pkm => {
            let inbox = store.create_folder_tx(tx, "Inbox", None).await?;
            store.create_folder_tx(tx, "Fleeting", None).await?;
            store.create_folder_tx(tx, "Literature", None).await?;
            store.create_folder_tx(tx, "Permanent", None).await?;
            store.create_folder_tx(tx, "Maps of content", None).await?;

            let body = "## PKM starter\n\n\
                - Capture quick ideas in **Inbox** or **Fleeting**.\n\
                - Move distilled notes into **Permanent** with `[[wiki-links]]` between concepts.\n\
                - Use **Literature** for sources and **Maps of content** for index notes.\n";
            let id = store
                .insert_note_full_tx(tx, "Welcome — PKM layout", body, Some(inbox))
                .await?;
            indexed.push((id, "Welcome — PKM layout".into(), body.into()));
        }
        StarterPackId::Para => {
            let projects = store.create_folder_tx(tx, "Projects", None).await?;
            let areas = store.create_folder_tx(tx, "Areas", None).await?;
            let resources = store.create_folder_tx(tx, "Resources", None).await?;
            let archives = store.create_folder_tx(tx, "Archives", None).await?;

            for (fid, title, body) in [
                (
                    projects,
                    "Projects — README",
                    "## Projects\n\nActive outcomes with a deadline. Move here when something is in motion.\n",
                ),
                (
                    areas,
                    "Areas — README",
                    "## Areas\n\nOngoing standards to maintain (health, finance, home) without a fixed end date.\n",
                ),
                (
                    resources,
                    "Resources — README",
                    "## Resources\n\nReference material you may need later: topics, tools, assets.\n",
                ),
                (
                    archives,
                    "Archives — README",
                    "## Archives\n\nInactive items from the other three top-level buckets.\n",
                ),
            ] {
                let id = store.insert_note_full_tx(tx, title, body, Some(fid)).await?;
                indexed.push((id, title.into(), body.into()));
            }
        }
        StarterPackId::BulletJournal => {
            let collections = store.create_folder_tx(tx, "Collections", None).await?;
            store.create_folder_tx(tx, "Future log", None).await?;
            store.create_folder_tx(tx, "Monthly", None).await?;

            let body = "## Bullet journal starter\n\n\
                - **Collections**: running lists and trackers.\n\
                - **Future log**: events beyond this month.\n\
                - **Monthly**: one note per month (create from the activity bar when you like).\n\
                - Daily notes use the calendar / daily note actions in the app.\n";
            let id = store
                .insert_note_full_tx(tx, "Welcome — bullet journal layout", body, Some(collections))
                .await?;
            indexed.push((id, "Welcome — bullet journal layout".into(), body.into()));
        }
    }

    Ok(indexed)
}
