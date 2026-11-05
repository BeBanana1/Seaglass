//! Liens vers le dépôt du projet (#188), ouverts dans le navigateur du système par
//! `tauri-plugin-opener`. Les URLs sont fixes ici : l'UI ne choisit qu'une page, jamais
//! une adresse, et n'a aucune permission du plugin.

use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use super::{CmdResult, err};

const REPO: &str = "https://git.bebanana.ch/bebanana/Seaglass";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectPage {
    /// Code source.
    Repo,
    /// Nouvelle issue : signaler un bug ou proposer une idée.
    NewIssue,
}

impl ProjectPage {
    fn url(self) -> String {
        match self {
            Self::Repo => REPO.to_owned(),
            Self::NewIssue => format!("{REPO}/issues/new"),
        }
    }
}

#[tauri::command]
pub async fn open_project_page(app: AppHandle, page: ProjectPage) -> CmdResult<()> {
    app.opener().open_url(page.url(), None::<&str>).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pages_map_to_fixed_urls() {
        let page: ProjectPage = serde_json::from_str(r#""newIssue""#).unwrap();
        assert_eq!(
            page.url(),
            "https://git.bebanana.ch/bebanana/Seaglass/issues/new"
        );
        assert_eq!(
            ProjectPage::Repo.url(),
            "https://git.bebanana.ch/bebanana/Seaglass"
        );
        assert!(serde_json::from_str::<ProjectPage>(r#""https://evil""#).is_err());
    }
}
