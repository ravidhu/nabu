use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

pub struct KnownPaths {
    pub binary: Option<PathBuf>,
    pub nabu_home: PathBuf,
    pub uv: PathBuf,
    pub transcribe: PathBuf,
    pub hf_cache: PathBuf,
    pub wespeaker: PathBuf,
    pub sessions: PathBuf,
    pub transcribe_env_override: Option<PathBuf>,
    pub hf_token_set: bool,
}

pub fn resolve() -> Result<KnownPaths> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?;
    let nabu_home = home.join(".nabu");

    Ok(KnownPaths {
        binary: std::env::current_exe().ok(),
        uv: nabu_home.join("bin/uv"),
        transcribe: nabu_home.join("transcribe"),
        hf_cache: home.join(".cache/huggingface/hub"),
        wespeaker: home.join(".wespeaker/english"),
        sessions: home.join("nabu_data"),
        nabu_home,
        transcribe_env_override: std::env::var("NABU_TRANSCRIBE_DIR").ok().map(PathBuf::from),
        hf_token_set: std::env::var("HF_TOKEN").is_ok(),
    })
}

/// Sum the size of every regular file under `path`. Returns 0 if `path` is missing.
pub fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = fs::read_dir(path) else { return 0 };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if meta.is_symlink() {
            continue;
        }
        if meta.is_file() {
            total += meta.len();
        } else if meta.is_dir() {
            total += dir_size(&path);
        }
    }
    total
}

pub fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    let size = bytes as f64;
    if size < KB {
        format!("{bytes} B")
    } else if size < KB * KB {
        format!("{:.1} KB", size / KB)
    } else if size < KB * KB * KB {
        format!("{:.1} MB", size / (KB * KB))
    } else {
        format!("{:.2} GB", size / (KB * KB * KB))
    }
}

fn fmt_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            return format!("~/{}", rel.display());
        }
    }
    path.display().to_string()
}

fn fmt_present(path: &Path) -> String {
    if !path.exists() {
        return "(missing)".into();
    }
    if path.is_dir() {
        return format!("[{}]", human_size(dir_size(path)));
    }
    if let Ok(meta) = fs::metadata(path) {
        return format!("[{}]", human_size(meta.len()));
    }
    "(present)".into()
}

fn count_sessions(sessions: &Path) -> usize {
    let Ok(entries) = fs::read_dir(sessions) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            entry.path().is_dir()
                && entry
                    .file_name()
                    .to_str()
                    .map_or(false, |name| !name.starts_with('.'))
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn dir_size_sums_files_recursively() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();
        File::create(tmp.path().join("a/one.bin"))
            .unwrap()
            .write_all(&[0u8; 100])
            .unwrap();
        File::create(nested.join("two.bin"))
            .unwrap()
            .write_all(&[0u8; 250])
            .unwrap();
        assert_eq!(dir_size(tmp.path()), 350);
    }

    #[test]
    fn dir_size_missing_returns_zero() {
        assert_eq!(dir_size(Path::new("/this/path/does/not/exist/xyz")), 0);
    }

    #[test]
    fn human_size_thresholds() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2 * 1024), "2.0 KB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert!(human_size(3 * 1024 * 1024 * 1024).starts_with("3."));
        assert!(human_size(3 * 1024 * 1024 * 1024).ends_with("GB"));
    }
}

pub fn print() -> Result<()> {
    let paths = resolve()?;
    let row = |label: &str, path: &str, extra: &str| {
        println!("  {:<14} {:<48} {}", label, path, extra);
    };

    println!("nabu paths");
    println!();
    if let Some(bin) = &paths.binary {
        row("binary", &fmt_path(bin), &fmt_present(bin));
    }
    row(
        "nabu home",
        &fmt_path(&paths.nabu_home),
        &fmt_present(&paths.nabu_home),
    );
    row("uv runtime", &fmt_path(&paths.uv), &fmt_present(&paths.uv));
    row(
        "transcribe",
        &fmt_path(&paths.transcribe),
        &fmt_present(&paths.transcribe),
    );
    row(
        "hf cache",
        &fmt_path(&paths.hf_cache),
        &fmt_present(&paths.hf_cache),
    );
    row(
        "wespeaker",
        &fmt_path(&paths.wespeaker),
        &fmt_present(&paths.wespeaker),
    );
    let sess_extra = if paths.sessions.exists() {
        format!(
            "{}  ({} session{})",
            fmt_present(&paths.sessions),
            count_sessions(&paths.sessions),
            if count_sessions(&paths.sessions) == 1 {
                ""
            } else {
                "s"
            }
        )
    } else {
        "(missing)".into()
    };
    row("sessions", &fmt_path(&paths.sessions), &sess_extra);

    println!();
    println!("environment");
    println!(
        "  NABU_TRANSCRIBE_DIR = {}",
        paths
            .transcribe_env_override
            .as_ref()
            .map(|dir| dir.display().to_string())
            .unwrap_or_else(|| "(not set)".into())
    );
    println!(
        "  HF_TOKEN            = {}",
        if paths.hf_token_set {
            "(set — value hidden)"
        } else {
            "(not set)"
        }
    );

    Ok(())
}
