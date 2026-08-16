use std::fs;
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use chrono::Local;

pub struct SessionPaths {
    pub write_dir: PathBuf,
    pub final_dir: PathBuf,
}

impl SessionPaths {
    pub fn mic_write(&self) -> PathBuf {
        self.write_dir.join("mic.wav")
    }
    pub fn sys_write(&self) -> PathBuf {
        self.write_dir.join("system.wav")
    }
    pub fn merged_write(&self) -> PathBuf {
        self.write_dir.join("merged.wav")
    }
    pub fn using_tmp(&self) -> bool {
        self.write_dir != self.final_dir
    }
}

/// Resolve the session output directory.
///
/// When `out` is given it is used directly. Otherwise a timestamped folder is
/// created under `~/nabu_data/`, written to `.tmp/` during recording and
/// atomically renamed on clean exit.
pub fn resolve(out: Option<PathBuf>) -> Result<SessionPaths> {
    if let Some(dir) = out {
        fs::create_dir_all(&dir).context("create output directory")?;
        return Ok(SessionPaths {
            write_dir: dir.clone(),
            final_dir: dir,
        });
    }

    let base = dirs::home_dir()
        .ok_or_else(|| anyhow!("cannot determine home directory"))?
        .join("nabu_data");
    let tmp_base = base.join(".tmp");
    fs::create_dir_all(&base).context("create ~/nabu_data")?;
    fs::create_dir_all(&tmp_base).context("create ~/nabu_data/.tmp")?;

    let name = Local::now().format("%Y_%m_%d_%H_%M").to_string();
    let write_dir = tmp_base.join(&name);
    let final_dir = base.join(&name);
    fs::create_dir_all(&write_dir).context("create session tmp dir")?;

    Ok(SessionPaths { write_dir, final_dir })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_out_dir_skips_tmp() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("my-session");
        let session = resolve(Some(out.clone())).unwrap();
        assert_eq!(session.write_dir, out);
        assert_eq!(session.final_dir, out);
        assert!(!session.using_tmp());
        assert_eq!(session.mic_write(), out.join("mic.wav"));
        assert_eq!(session.sys_write(), out.join("system.wav"));
        assert_eq!(session.merged_write(), out.join("merged.wav"));
        assert!(out.exists());
    }
}
