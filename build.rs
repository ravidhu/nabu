use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    let root    = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Bake the source-tree path for dev mode (NABU_TRANSCRIBE_DIR env override).
    println!("cargo:rustc-env=NABU_TRANSCRIBE_DIR={root}/transcribe");

    // ── Bundle transcribe/ Python sources ────────────────────────────────────
    // Exclude .venv / __pycache__ / *.pyc — uv recreates the venv on first run.
    let archive = out_dir.join("transcribe.tar.gz");
    let t_src   = format!("{root}/transcribe");
    let status  = Command::new("tar")
        .args([
            "-czf", archive.to_str().unwrap(),
            "--exclude=.venv",
            "--exclude=__pycache__",
            "--exclude=*.pyc",
            "-C", &t_src, ".",
        ])
        .status()
        .expect("tar not found — required to build nabu");
    assert!(status.success(), "failed to bundle transcribe/");

    // ── Download uv binary (macOS aarch64) ───────────────────────────────────
    // Cached in OUT_DIR between builds; only re-downloaded after cargo clean.
    let uv_path = out_dir.join("uv");
    if !uv_path.exists() {
        eprintln!("[nabu build] downloading uv binary …");
        let tar  = out_dir.join("uv-dl.tar.gz");
        let url  = "https://github.com/astral-sh/uv/releases/latest/download/uv-aarch64-apple-darwin.tar.gz";
        let ok   = Command::new("curl")
            .args(["-fsSL", "-o", tar.to_str().unwrap(), url])
            .status()
            .expect("curl not found")
            .success();
        assert!(ok, "failed to download uv from GitHub releases");

        // Extract to a temp dir first — handle both flat and subdirectory layouts.
        let tmp = out_dir.join("uv-extract");
        fs::create_dir_all(&tmp).unwrap();
        Command::new("tar")
            .args(["-xzf", tar.to_str().unwrap(), "-C", tmp.to_str().unwrap()])
            .status()
            .expect("tar extraction failed");
        fs::remove_file(&tar).ok();

        // uv binary may be at root or one directory deep.
        let candidate = tmp.join("uv");
        let src = if candidate.exists() {
            candidate
        } else {
            tmp.join("uv-aarch64-apple-darwin").join("uv")
        };
        fs::copy(&src, &uv_path).expect("could not copy uv binary");
        fs::remove_dir_all(&tmp).ok();
        eprintln!("[nabu build] ✓ uv bundled");
    }

    // Re-bundle whenever Python sources change.
    println!("cargo:rerun-if-changed=transcribe/transcribe.py");
    println!("cargo:rerun-if-changed=transcribe/pyproject.toml");
    println!("cargo:rerun-if-changed=transcribe/uv.lock");
    println!("cargo:rerun-if-changed=build.rs");
}
