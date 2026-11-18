//! Version log in the app (#340): `CHANGELOG.md` embedded at build time, split into
//! versions and filtered for the current platform. Log convention: an entry starting with
//! `Android:`, `Windows:`, `Linux:` or `Desktop:` only concerns that platform; `chore:`
//! entries are not shown.

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
    /// Publication date (`YYYY-MM-DD`) of a tagged version.
    pub published: Option<String>,
    pub entries: Vec<String>,
}

/// Entry kept for `platform`, without its platform prefix.
fn entry_for(text: &str, platform: Platform) -> Option<&str> {
    if text.starts_with("chore:") {
        return None;
    }
    let prefixes: [(&str, &[Platform]); 4] = [
        ("Android:", &[Platform::Android]),
        ("Windows:", &[Platform::Windows]),
        ("Linux:", &[Platform::Linux]),
        ("Desktop:", &[Platform::Windows, Platform::Linux]),
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
                .strip_prefix("(published ")
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
    // A version with nothing for this platform: no point showing it.
    releases.retain(|r| !r.entries.is_empty());
    releases
}

/// Versions from most recent to oldest, with the current platform's entries.
#[tauri::command]
pub async fn changelog() -> CmdResult<Vec<Release>> {
    Ok(parse(CHANGELOG, CURRENT))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\n- not an entry\n\n## 0.2.0\n\n- Shared (#1)\n- Android: touch (#2)\n- Desktop: window (#3)\n\n## 0.1.1 (published 2026-11-06)\n\n- Windows: SMTC\n- chore: version\n";

    #[test]
    fn filters_by_platform_and_drops_empty_versions() {
        let android = parse(SAMPLE, Platform::Android);
        assert_eq!(android.len(), 1);
        assert_eq!(android[0].entries, ["Shared (#1)", "touch (#2)"]);

        let windows = parse(SAMPLE, Platform::Windows);
        assert_eq!(windows[0].entries, ["Shared (#1)", "window (#3)"]);
        assert_eq!(windows[1].version, "0.1.1");
        assert_eq!(windows[1].published.as_deref(), Some("2026-11-06"));
        assert_eq!(windows[1].entries, ["SMTC"]);

        let linux = parse(SAMPLE, Platform::Linux);
        assert_eq!(linux.len(), 1);
    }

    #[test]
    fn real_changelog_starts_with_current_version() {
        // First title of the log, unfiltered: a version that only touches one platform
        // does not appear in the other platforms' log (#342).
        let first = CHANGELOG
            .lines()
            .find_map(|line| line.strip_prefix("## "))
            .and_then(|title| title.split(' ').next());
        assert_eq!(first, Some(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn real_changelog_uses_the_english_markers() {
        // A leftover French marker would show its prefix in the app or lose the date.
        for line in CHANGELOG.lines() {
            assert!(!line.contains("(publiée le "), "French date marker: {line}");
            for prefix in ["- Android :", "- Windows :", "- Linux :", "- Bureau :"] {
                assert!(!line.starts_with(prefix), "French prefix: {line}");
            }
        }
    }
}
