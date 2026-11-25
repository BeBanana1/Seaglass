//! Links to the project's pages (#188, website #517), opened in the system browser by
//! `tauri-plugin-opener`. The URLs are fixed here: the UI only picks a page, never an
//! address, and has no permission of the plugin.

use serde::Deserialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use super::{CmdResult, err};

const REPO: &str = "https://github.com/BeBanana1/Seaglass";
const WEBSITE: &str = "https://seaglass.bebanana.ch";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectPage {
    /// Project website: presentation and downloads.
    Website,
    /// Source code.
    Repo,
    /// New issue: report a bug or suggest an idea.
    NewIssue,
    /// README section on the TIDAL client identifier built into the app (#38).
    ClientId,
}

impl ProjectPage {
    fn url(self) -> String {
        match self {
            Self::Website => WEBSITE.to_owned(),
            Self::Repo => REPO.to_owned(),
            Self::NewIssue => format!("{REPO}/issues/new/choose"),
            Self::ClientId => format!("{REPO}#tidal-client-identifiers"),
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
            "https://github.com/BeBanana1/Seaglass/issues/new/choose"
        );
        assert_eq!(
            ProjectPage::Repo.url(),
            "https://github.com/BeBanana1/Seaglass"
        );
        let page: ProjectPage = serde_json::from_str(r#""website""#).unwrap();
        assert_eq!(page.url(), "https://seaglass.bebanana.ch");
        let page: ProjectPage = serde_json::from_str(r#""clientId""#).unwrap();
        assert_eq!(
            page.url(),
            "https://github.com/BeBanana1/Seaglass#tidal-client-identifiers"
        );
        assert!(serde_json::from_str::<ProjectPage>(r#""https://evil""#).is_err());
    }
}
