//! Environnement de mpv lancé depuis l'AppImage (#320).
//!
//! L'`AppRun` de l'AppImage pose des variables qui pointent dans son point de montage
//! (`LD_LIBRARY_PATH`, `GST_PLUGIN_*`, `GTK_*`, `XDG_DATA_DIRS`, `PATH`…). mpv est le
//! binaire du système : avec ces variables, il chargeait les bibliothèques de l'AppImage
//! (95 avec `LD_LIBRARY_PATH` seul), construites pour une autre distribution. Après une
//! relance (mise à jour, #314), l'environnement garde aussi les chemins de l'ancien point
//! de montage. On retire ces entrées pour mpv seulement : l'app, elle, en a besoin.

use std::path::Path;

/// Correction à appliquer à une variable de l'environnement de mpv.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    Remove(String),
    Set(String, String),
}

/// Corrections pour un environnement hérité de l'AppImage montée en `appdir`. Les listes
/// de chemins (séparées par `:`) perdent leurs entrées venues d'une AppImage ; une variable
/// qui n'en contenait pas d'autres est retirée. Les autres variables ne changent pas.
pub fn fixes<I>(vars: I, appdir: &str) -> Vec<Fix>
where
    I: IntoIterator<Item = (String, String)>,
{
    let appdir = appdir.trim_end_matches('/');
    if appdir.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<Fix> = vars
        .into_iter()
        .filter_map(|(key, value)| {
            let entries: Vec<&str> = value.split(':').collect();
            if !entries.iter().any(|e| from_appimage(e, appdir)) {
                return None;
            }
            // Les entrées vides (`a::b`, `:` final) valent « dossier courant » pour
            // `LD_LIBRARY_PATH` et `PATH` : jamais gardées.
            let kept: Vec<&str> = entries
                .into_iter()
                .filter(|e| !e.is_empty() && !from_appimage(e, appdir))
                .collect();
            Some(if kept.is_empty() {
                Fix::Remove(key)
            } else {
                Fix::Set(key, kept.join(":"))
            })
        })
        .collect();
    out.sort_by(|a, b| key(a).cmp(key(b)));
    out
}

fn key(fix: &Fix) -> &str {
    match fix {
        Fix::Remove(k) | Fix::Set(k, _) => k,
    }
}

/// Chemin situé dans l'AppImage courante, ou dans un point de montage d'AppImage
/// (`…/.mount_<nom><aléa>`, celui d'avant une relance).
fn from_appimage(entry: &str, appdir: &str) -> bool {
    let entry = entry.trim();
    if entry == appdir || entry.starts_with(&format!("{appdir}/")) {
        return true;
    }
    Path::new(entry).components().any(|c| {
        c.as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with(".mount_"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPDIR: &str = "/tmp/.mount_SeaglaNknJPa";

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn appimage_entries_are_dropped_from_path_lists() {
        let vars = env(&[
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_SeaglaNknJPa/usr/lib/:/tmp/.mount_SeaglaNknJPa/usr/lib/x86_64-linux-gnu/",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_SeaglaNknJPa/usr/share/:/usr/local/share:/usr/share:",
            ),
            (
                "PATH",
                "/tmp/.mount_SeaglaNknJPa/usr/bin/:/usr/local/bin:/usr/bin",
            ),
        ]);
        assert_eq!(
            fixes(vars, APPDIR),
            vec![
                Fix::Remove("LD_LIBRARY_PATH".into()),
                Fix::Set("PATH".into(), "/usr/local/bin:/usr/bin".into()),
                Fix::Set("XDG_DATA_DIRS".into(), "/usr/local/share:/usr/share".into()),
            ]
        );
    }

    #[test]
    fn single_paths_into_the_appimage_are_removed() {
        let vars = env(&[
            ("APPDIR", APPDIR),
            ("GTK_DATA_PREFIX", "/tmp/.mount_SeaglaNknJPa"),
            (
                "GDK_PIXBUF_MODULE_FILE",
                "/tmp/.mount_SeaglaNknJPa//usr/lib/gdk-pixbuf-2.0/2.10.0/loaders.cache",
            ),
            ("PYTHONHOME", "/tmp/.mount_SeaglaNknJPa/usr/"),
        ]);
        let mut removed: Vec<Fix> = fixes(vars, APPDIR);
        removed.retain(|f| matches!(f, Fix::Remove(_)));
        assert_eq!(removed.len(), 4);
    }

    #[test]
    fn mounts_left_by_a_restart_are_dropped_too() {
        let vars = env(&[(
            "GST_PLUGIN_SYSTEM_PATH",
            "/tmp/.mount_SeaglaNknJPa/usr/lib/gstreamer:/tmp/.mount_SeaglaKOBHeA/usr/lib/qt4/plugins/",
        )]);
        assert_eq!(
            fixes(vars, APPDIR),
            vec![Fix::Remove("GST_PLUGIN_SYSTEM_PATH".into())]
        );
    }

    #[test]
    fn unrelated_variables_are_kept() {
        let vars = env(&[
            ("HOME", "/home/u"),
            ("GTK_THEME", "Adwaita:light"),
            ("PULSE_SERVER", "unix:/mnt/wslg/PulseServer"),
            ("APPIMAGE", "/home/u/Apps/Seaglass.AppImage"),
            // Des noms proches, sans être un point de montage d'AppImage.
            ("A", "/tmp/.mountain/x"),
            ("B", "/tmp/mount_Seaglass/lib"),
        ]);
        assert!(fixes(vars, APPDIR).is_empty());
    }

    #[test]
    fn empty_appdir_changes_nothing() {
        let vars = env(&[("LD_LIBRARY_PATH", "/tmp/.mount_X/usr/lib")]);
        assert!(fixes(vars, "").is_empty());
    }
}
