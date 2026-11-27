//! Log on disk (#192): `seaglass.log` in the app's log folder, in addition to
//! the standard output (lost on Android and in an installed build).
//!
//! The previous launch becomes `seaglass.old.log` at startup; beyond
//! `MAX_BYTES`, the current log goes there too (Android keeps the process for days).
//! URL parameters are masked: a stream URL is signed.

use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tracing_subscriber::fmt::MakeWriter;

pub const LOG_FILE: &str = "seaglass.log";
pub const OLD_LOG_FILE: &str = "seaglass.old.log";
const MAX_BYTES: u64 = 4 * 1024 * 1024;

struct Sink {
    dir: PathBuf,
    file: File,
    written: u64,
}

static SINK: Mutex<Option<Sink>> = Mutex::new(None);

thread_local! {
    /// True during a write: a panic in `write` (hook included) does not go back through
    /// the lock it already holds.
    static WRITING: Cell<bool> = const { Cell::new(false) };
}

/// Opens this launch's log; until then, only the standard output receives the lines.
pub fn open(dir: &Path) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let file = rotate(dir)?;
    if let Ok(mut sink) = SINK.lock() {
        *sink = Some(Sink {
            dir: dir.to_owned(),
            file,
            written: 0,
        });
    }
    Ok(())
}

/// `seaglass.log` becomes `seaglass.old.log` (replaced), then an empty log is opened.
fn rotate(dir: &Path) -> io::Result<File> {
    let current = dir.join(LOG_FILE);
    if current.exists() {
        fs::rename(&current, dir.join(OLD_LOG_FILE))?;
    }
    OpenOptions::new().create(true).append(true).open(current)
}

/// Destination of the `tracing` lines; a write error is never reported.
pub struct FileWriter;

impl Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if WRITING.with(|w| w.replace(true)) {
            return Ok(buf.len());
        }
        write_line(buf);
        WRITING.with(|w| w.set(false));
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `tracing` writes each event in a single call: the whole line goes through `redact`.
fn write_line(buf: &[u8]) {
    let Ok(mut guard) = SINK.lock() else {
        return;
    };
    let Some(sink) = guard.as_mut() else {
        return;
    };
    if sink.written >= MAX_BYTES {
        match rotate(&sink.dir) {
            Ok(file) => {
                sink.file = file;
                sink.written = 0;
            }
            Err(_) => return,
        }
    }
    let line = redact(&String::from_utf8_lossy(buf));
    // No buffer: with `panic = "abort"`, nothing written must stay in memory.
    if sink.file.write_all(line.as_bytes()).is_ok() {
        sink.written += line.len() as u64;
    }
}

impl<'a> MakeWriter<'a> for FileWriter {
    type Writer = FileWriter;

    fn make_writer(&'a self) -> Self::Writer {
        FileWriter
    }
}

/// Masks the URL parameters (`https://h/p?token=…` → `https://h/p?…`): stream URLs
/// are signed, those of the API carry the country and sometimes identifiers.
pub fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = find_url(rest) {
        let (before, url) = rest.split_at(start);
        out.push_str(before);
        let end = url
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | '>' | ']'))
            .unwrap_or(url.len());
        let (url, after) = url.split_at(end);
        match url.find('?') {
            Some(q) => {
                out.push_str(&url[..=q]);
                out.push('…');
            }
            None => out.push_str(url),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn find_url(text: &str) -> Option<usize> {
    match (text.find("http://"), text.find("https://")) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// The last `max` lines of a log, if it exists.
pub fn tail(path: &Path, max: usize) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = text.lines().collect();
    let from = lines.len().saturating_sub(max);
    Some(lines[from..].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_hides_query_strings() {
        assert_eq!(
            redact("échec sur https://sp.example/a.flac?token=abc&exp=1 (403)"),
            "échec sur https://sp.example/a.flac?… (403)"
        );
        assert_eq!(
            redact("\"http://x/y?a=1\" et https://z/w"),
            "\"http://x/y?…\" et https://z/w"
        );
        assert_eq!(redact("rien à masquer"), "rien à masquer");
    }

    /// Only test that touches the global log `SINK`.
    #[test]
    fn writer_redacts_and_rotates_at_open() {
        let dir = std::env::temp_dir().join(format!("seaglass-log-{}", std::process::id()));
        open(&dir).unwrap();
        FileWriter
            .write_all(b"lecture de https://cdn/x.flac?sig=secret\n")
            .unwrap();
        let first = fs::read_to_string(dir.join(LOG_FILE)).unwrap();
        assert_eq!(first, "lecture de https://cdn/x.flac?…\n");
        open(&dir).unwrap();
        assert_eq!(fs::read_to_string(dir.join(OLD_LOG_FILE)).unwrap(), first);
        assert_eq!(fs::read_to_string(dir.join(LOG_FILE)).unwrap(), "");
        if let Ok(mut sink) = SINK.lock() {
            *sink = None;
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tail_keeps_last_lines() {
        let dir = std::env::temp_dir().join(format!("seaglass-tail-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.log");
        fs::write(&path, "a\nb\nc\nd\n").unwrap();
        assert_eq!(tail(&path, 2).as_deref(), Some("c\nd"));
        assert_eq!(tail(&path, 10).as_deref(), Some("a\nb\nc\nd"));
        assert_eq!(tail(&dir.join("absent.log"), 2), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
