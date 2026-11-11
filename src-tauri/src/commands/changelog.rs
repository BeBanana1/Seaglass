//! Journal des versions dans l'app (#340) : `CHANGELOG.md` embarqué à la compilation,
//! découpé en versions et filtré pour la plateforme courante. Convention du journal :
//! une entrée qui commence par `Android :`, `Windows :`, `Linux :` ou `Bureau :` ne
//! concerne que cette plateforme ; les entrées `chore:` ne s'affichent pas.

use serde::Serialize;

use super::CmdResult;

const CHANGELOG: &str = include_str!("../../../CHANGELOG.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Windows,
    Linux,
    Android,
}

const CURRENT: Platform = if cfg!(target_os = "android") {
    Platform::Android
} else if cfg!(windows) {
    Platform::Windows
} else {
    Platform::Linux
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    /// Date de publication (`AAAA-MM-JJ`) d'une version taguée.
    pub published: Option<String>,
    pub entries: Vec<String>,
}

/// Entrée gardée pour `platform`, sans son préfixe de plateforme.
fn entry_for(text: &str, platform: Platform) -> Option<&str> {
    if text.starts_with("chore:") {
        return None;
    }
    let prefixes: [(&str, &[Platform]); 4] = [
        ("Android :", &[Platform::Android]),
        ("Windows :", &[Platform::Windows]),
        ("Linux :", &[Platform::Linux]),
        ("Bureau :", &[Platform::Windows, Platform::Linux]),
    ];
    for (prefix, targets) in prefixes {
        if let Some(rest) = text.strip_prefix(prefix) {
            return targets.contains(&platform).then_some(rest.trim_start());
        }
    }
    Some(text)
}

fn parse(text: &str, platform: Platform) -> Vec<Release> {
    let mut releases: Vec<Release> = Vec::new();
    for line in text.lines() {
        if let Some(title) = line.strip_prefix("## ") {
            let (version, rest) = title.split_once(' ').unwrap_or((title, ""));
            let published = rest
                .strip_prefix("(publiée le ")
                .and_then(|r| r.strip_suffix(')'))
                .map(str::to_owned);
            releases.push(Release {
                version: version.to_owned(),
                published,
                entries: Vec::new(),
            });
        } else if let (Some(entry), Some(release)) = (line.strip_prefix("- "), releases.last_mut())
            && let Some(kept) = entry_for(entry, platform)
        {
            release.entries.push(kept.to_owned());
        }
    }
    // Version sans rien pour cette plateforme : inutile à afficher.
    releases.retain(|r| !r.entries.is_empty());
    releases
}

/// Versions de la plus récente à la plus ancienne, entrées de la plateforme courante.
#[tauri::command]
pub async fn changelog() -> CmdResult<Vec<Release>> {
    Ok(parse(CHANGELOG, CURRENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Journal\n\n- pas une entrée\n\n## 0.2.0\n\n- Commun (#1)\n- Android : tactile (#2)\n- Bureau : fenêtre (#3)\n\n## 0.1.1 (publiée le 2026-11-06)\n\n- Windows : SMTC\n- chore: version\n";

    #[test]
    fn filters_by_platform_and_drops_empty_versions() {
        let android = parse(SAMPLE, Platform::Android);
        assert_eq!(android.len(), 1);
        assert_eq!(android[0].entries, ["Commun (#1)", "tactile (#2)"]);

        let windows = parse(SAMPLE, Platform::Windows);
        assert_eq!(windows[0].entries, ["Commun (#1)", "fenêtre (#3)"]);
        assert_eq!(windows[1].version, "0.1.1");
        assert_eq!(windows[1].published.as_deref(), Some("2026-11-06"));
        assert_eq!(windows[1].entries, ["SMTC"]);

        let linux = parse(SAMPLE, Platform::Linux);
        assert_eq!(linux.len(), 1);
    }

    #[test]
    fn real_changelog_starts_with_current_version() {
        // Premier titre du journal, sans filtre : une version qui ne touche qu'une
        // plateforme n'apparaît pas dans le journal des autres (#342).
        let first = CHANGELOG
            .lines()
            .find_map(|line| line.strip_prefix("## "))
            .and_then(|title| title.split(' ').next());
        assert_eq!(first, Some(env!("CARGO_PKG_VERSION")));
    }
}
