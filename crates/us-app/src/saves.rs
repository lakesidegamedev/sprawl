//! Saved games on disk: where they live, the list the load screen shows, and the autosave.
//!
//! A save file is the simulation's own bytes followed by a short text trailer (name, time, town,
//! date, population). The simulation reader ignores the trailer, so every file is still a plain
//! `Sim::save` file, and the load screen can list saves without rebuilding each town.

use std::path::{Path, PathBuf};
use us_sim::{AUTOSAVE_SLOTS, LoadError, SAVE_VERSION, Sim, autosave_path};

const TRAILER_TAG: &[u8] = b"\nUSMETA1\n";
const EXT: &str = "save";

/// The folder saves live in: the standard per-user data folder, outside the install folder, so
/// an update never touches them. `URBAN_SPRAWL_SAVE_DIR` overrides it (tests, screenshots).
pub fn save_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("URBAN_SPRAWL_SAVE_DIR") {
        return PathBuf::from(d);
    }
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    #[cfg(target_os = "ios")]
    let base = home().map(|h| h.join("Documents"));
    #[cfg(target_os = "windows")]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = home().map(|h| h.join("Library").join("Application Support"));
    #[cfg(not(any(target_os = "ios", target_os = "windows", target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(".local").join("share")));
    base.unwrap_or_else(std::env::temp_dir)
        .join("Urban Sprawl")
        .join("saves")
}

/// What the load screen shows about one file.
#[derive(Clone, Debug)]
pub struct SaveInfo {
    pub path: PathBuf,
    pub name: String,
    /// Seconds since 1970 when it was written.
    pub unix: u64,
    pub town: String,
    pub date: String,
    pub population: i64,
    pub sandbox: bool,
    pub auto: bool,
    /// Set when the file cannot be opened by this version, in plain words.
    pub problem: Option<String>,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// What a trailer records about a game.
pub struct Meta<'a> {
    pub name: &'a str,
    pub auto: bool,
}

fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// The bytes to write for `sim`: its save plus the trailer.
pub fn encode(sim: &mut Sim, meta: Meta) -> Vec<u8> {
    let mut b = sim.save();
    let rules = sim.rules();
    let t = sim.town();
    let text = format!(
        "name={}\nunix={}\ntown={}\ndate={}\npop={}\nsandbox={}\nauto={}\n",
        clean(meta.name),
        now_unix(),
        t.preset.label(),
        sim.date().text(),
        sim.population(),
        rules.is_sandbox() as u8,
        meta.auto as u8,
    );
    b.extend_from_slice(TRAILER_TAG);
    b.extend_from_slice(text.as_bytes());
    b
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

fn parse(path: &Path, bytes: &[u8]) -> SaveInfo {
    let mut info = SaveInfo {
        path: path.to_path_buf(),
        name: path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        unix: 0,
        town: String::new(),
        date: String::new(),
        population: 0,
        sandbox: false,
        auto: false,
        problem: None,
    };
    if bytes.len() < 6 || &bytes[..4] != b"USSV" {
        info.problem = Some(LoadError::NotASave.to_string());
        return info;
    }
    let v = u16::from_le_bytes([bytes[4], bytes[5]]);
    if v > SAVE_VERSION {
        info.problem = Some(
            LoadError::TooNew {
                found: v,
                current: SAVE_VERSION,
            }
            .to_string(),
        );
    } else if v < SAVE_VERSION {
        info.problem = Some(
            LoadError::TooOld {
                found: v,
                current: SAVE_VERSION,
            }
            .to_string(),
        );
    }
    if let Some(at) = find(bytes, TRAILER_TAG) {
        let text = String::from_utf8_lossy(&bytes[at + TRAILER_TAG.len()..]);
        for line in text.lines() {
            let Some((k, val)) = line.split_once('=') else {
                continue;
            };
            match k {
                "name" if !val.is_empty() => info.name = val.to_string(),
                "unix" => info.unix = val.parse().unwrap_or(0),
                "town" => info.town = val.to_string(),
                "date" => info.date = val.to_string(),
                "pop" => info.population = val.parse().unwrap_or(0),
                "sandbox" => info.sandbox = val == "1",
                "auto" => info.auto = val == "1",
                _ => {}
            }
        }
    } else if info.problem.is_none() {
        // a plain simulation save with no trailer: still opens, but nothing to show about it
        info.town = "Unknown town".into();
    }
    info
}

/// Every save in `dir`, newest first. Unreadable files are listed with the reason.
pub fn list(dir: &Path) -> Vec<SaveInfo> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<SaveInfo> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == EXT))
        .filter_map(|p| {
            let bytes = std::fs::read(&p).ok()?;
            let mut info = parse(&p, &bytes);
            if info.unix == 0
                && let Ok(m) = std::fs::metadata(&p).and_then(|m| m.modified())
            {
                info.unix = m
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
            }
            if bytes.len() < 22 && info.problem.is_none() {
                info.problem = Some(LoadError::Truncated.to_string());
            }
            Some(info)
        })
        .collect();
    out.sort_by(|a, b| b.unix.cmp(&a.unix).then(a.path.cmp(&b.path)));
    out
}

/// A file name for a save called `name`: letters and digits, spaces to dashes.
pub fn file_stem(name: &str) -> String {
    let s: String = clean(name)
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c
            } else if c == ' ' || c == '-' || c == '_' {
                '-'
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "town".into()
    } else {
        s.chars().take(60).collect()
    }
}

/// Write the named save into `dir`. Writes a temporary file first so a crash never leaves half a
/// save. Returns the path.
pub fn write_named(dir: &Path, sim: &mut Sim, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let bytes = encode(sim, Meta { name, auto: false });
    let path = dir.join(format!("{}.{EXT}", file_stem(name)));
    let tmp = dir.join("write.tmp");
    std::fs::write(&tmp, &bytes)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// The yearly autosave: keeps the newest [`AUTOSAVE_SLOTS`] files (`autosave-1` is newest).
pub fn write_autosave(dir: &Path, sim: &mut Sim, town_name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let bytes = encode(
        sim,
        Meta {
            name: &format!("Autosave, {town_name}"),
            auto: true,
        },
    );
    for n in (1..AUTOSAVE_SLOTS).rev() {
        let from = autosave_path(dir, n);
        if from.exists() {
            std::fs::rename(&from, autosave_path(dir, n + 1))?;
        }
    }
    let tmp = dir.join("autosave.tmp");
    std::fs::write(&tmp, &bytes)?;
    let to = autosave_path(dir, 1);
    std::fs::rename(&tmp, &to)?;
    Ok(to)
}

/// Read and rebuild a save. The error text is fit to show the player.
pub fn load(path: &Path) -> Result<Sim, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read the save file: {e}"))?;
    Sim::load(&bytes).map_err(|e| e.to_string())
}

pub fn delete(path: &Path) -> std::io::Result<()> {
    std::fs::remove_file(path)
}

/// "3 Mar 2031 saved 2 days ago" style age text.
pub fn age_text(unix: u64) -> String {
    if unix == 0 {
        return String::new();
    }
    let secs = now_unix().saturating_sub(unix);
    let (n, unit) = if secs < 90 {
        return "just now".into();
    } else if secs < 3600 * 2 {
        (secs / 60, "minute")
    } else if secs < 86_400 * 2 {
        (secs / 3600, "hour")
    } else {
        (secs / 86_400, "day")
    };
    format!("{n} {unit}s ago")
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    fn small_sim() -> Sim {
        let town = generate(Preset::Grid, 3);
        let hall = us_sim::hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 3);
        for _ in 0..40 {
            sim.step_day();
        }
        sim
    }

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("us-saves-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn named_save_lists_and_loads() {
        let dir = tmp("named");
        let mut sim = small_sim();
        let p = write_named(&dir, &mut sim, "My first town!").unwrap();
        assert!(p.ends_with("My-first-town_.save"));
        let l = list(&dir);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].name, "My first town!");
        assert_eq!(l[0].town, "Grid");
        assert_eq!(l[0].population, sim.population());
        assert!(l[0].problem.is_none());
        let back = load(&p).unwrap();
        assert_eq!(back.day, sim.day);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn autosave_rotates_and_latest_finds_newest() {
        let dir = tmp("auto");
        let mut sim = small_sim();
        for _ in 0..4 {
            sim.step_day();
            write_autosave(&dir, &mut sim, "Grid").unwrap();
        }
        let n = std::fs::read_dir(&dir).unwrap().count();
        assert_eq!(n, AUTOSAVE_SLOTS);
        let latest = list(&dir)
            .into_iter()
            .find(|i| i.problem.is_none())
            .unwrap();
        assert!(latest.auto);
        assert!(latest.path.ends_with("autosave-1.save"));
        // and the plain simulation loader still reads it
        assert!(Sim::load_latest_autosave(&dir).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn damaged_and_foreign_files_are_reported() {
        let dir = tmp("bad");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("junk.save"), b"hello").unwrap();
        let mut sim = small_sim();
        let mut b = encode(
            &mut sim,
            Meta {
                name: "cut",
                auto: false,
            },
        );
        let cut = b.len() / 3;
        b.truncate(cut);
        std::fs::write(dir.join("cut.save"), &b).unwrap();
        let l = list(&dir);
        assert_eq!(l.len(), 2);
        assert!(
            l.iter()
                .find(|i| i.name == "junk")
                .unwrap()
                .problem
                .is_some()
        );
        let e = load(&dir.join("cut.save")).err().unwrap();
        assert!(e.contains("cut short") || e.contains("damaged"), "{e}");
        assert!(
            l.iter()
                .filter(|i| i.problem.is_none())
                .all(|i| i.name != "junk")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn newer_format_is_flagged() {
        let dir = tmp("new");
        std::fs::create_dir_all(&dir).unwrap();
        let mut b = b"USSV".to_vec();
        b.extend_from_slice(&(SAVE_VERSION + 1).to_le_bytes());
        b.extend_from_slice(&[0u8; 40]);
        std::fs::write(dir.join("future.save"), &b).unwrap();
        let l = list(&dir);
        assert!(l[0].problem.as_ref().unwrap().contains("newer version"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
