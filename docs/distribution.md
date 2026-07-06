# nabu — Packaging, Distribution & Installation

How nabu is packaged into a single binary, what ships in a release, and exactly
what happens on a user's machine from `curl … | bash` to the first transcript.

This is the **developer/maintainer** view of installation. End users don't need
it — point them at [`setup-guide.md`](./setup-guide.md) or the README instead.
For runtime internals see [`architecture.md`](./architecture.md).

---

## The one thing to understand: a self-contained binary

nabu ships as **one file** — `nabu-aarch64-apple-darwin` — with no Python, no
`pip install`, no runtime prerequisites for the end user. Everything the Python
pipeline needs is baked into the binary at compile time and unpacked on first
run.

Two things are embedded via `include_bytes!` (see `src/bootstrap.rs`):

| Embedded blob | Source | Produced by |
|---|---|---|
| `uv` | Astral's `uv` release for `aarch64-apple-darwin` | `build.rs` downloads it at compile time |
| `transcribe.tar.gz` | the whole `transcribe/` tree (minus `.venv`/`__pycache__`/`*.pyc`) | `build.rs` runs `tar` |

So the binary carries **both the Python sources and the tool that will build
their venv**. The only thing *not* shipped in the binary is the multi-GB ML
models — those are downloaded on first `--setup` (see [First run](#first-run-nabu---setup)).

```
                build time (build.rs)                 first run (bootstrap.rs)
   transcribe/  ──tar──►  transcribe.tar.gz  ─┐
                                              ├─include_bytes!─►  nabu binary  ──extract──►  ~/.nabu/
   uv release   ──curl─►  uv                 ─┘
```

---

## What ships in a GitHub Release

nabu is **not** on crates.io or Homebrew. A release is two artifacts uploaded to
the `v<version>` GitHub Release (tag derived from `Cargo.toml`):

1. **`nabu-aarch64-apple-darwin`** — the self-contained binary above.
2. **`voxceleb_resnet221_LM.tar.gz`** — the wespeaker diarization model, mirrored
   off our release so first-run setup doesn't depend on a third-party host.

Both are pushed by `make publish` (which runs `publish-binary` + `mirror-wespeaker`).
The `install.sh` and `uninstall.sh` scripts are served raw from `main` on GitHub
— they are not release assets.

### Cutting a release (maintainer)

```bash
# 1. Bump the version — this drives BOTH the re-extract trigger and the release tag.
#    Edit Cargo.toml:  version = "0.1.1"
# 2. Build + publish (tag v0.1.1 must already exist on GitHub).
make build          # → bin/nabu-aarch64-apple-darwin
make publish        # gh release upload: binary + wespeaker model to v0.1.1
```

> **Always bump `version` when the bundled Python changes.** The first-run
> re-extract only fires when the embedded version string differs from
> `~/.nabu/.version`. Ship new Python under an unchanged version and existing
> users keep running the *old* extracted scripts against the new binary. The
> version bump also re-points `make publish` at the matching `v<version>` tag.
> (Recent example: the `.python-version`/`requires-python` torch-3.14 fix lives
> inside `transcribe.tar.gz`, so it only reaches users after a version bump +
> republish — a rebuilt binary alone isn't enough.)

---

## The installer: `install.sh` step by step

`curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/install.sh | bash`

Guards first: it aborts unless `uname` is `Darwin` and `uname -m` is `arm64`
(macOS on Apple Silicon only). Then five steps:

| Step | What it does | Notes |
|---|---|---|
| **1. Download binary** | `curl` the latest release binary to a temp file, `chmod +x` | fails loudly if the repo/asset isn't reachable |
| **2. Install** | `sudo mv` it to `/usr/local/bin/nabu`, then `xattr -d com.apple.quarantine` | the one `sudo` prompt; quarantine removal stops Gatekeeper blocking an unsigned binary |
| **3. Download models** | `nabu --setup` | ~3 GB, one-time; see below |
| **4. Launcher app** | `osacompile` a tiny `nabu.app` into `~/Applications` that opens Terminal and runs `nabu` | double-click / Spotlight / Dock launch for non-terminal users |
| **5. Verify** | `nabu --doctor` | its exit code is captured (a first-run FAIL on Screen Recording is expected) and used to tailor the closing message |

The binary is **unsigned / un-notarized**. Removing the quarantine attribute in
step 2 is what lets it run without a Gatekeeper prompt; a browser-downloaded
binary that skips this will be blocked until the user clears it manually.

### Manual install / build-from-source

`install.sh` just automates the README's manual steps. Maintainers building
locally use the Makefile instead:

```bash
make dev-setup   # rustup target + uv sync + nabu --setup (first time)
make build       # release binary → bin/
make install     # build + sudo cp to /usr/local/bin + fresh --setup + nabu.app
```

`make install` does `rm -rf ~/.nabu` before `--setup` for a clean internal state;
`make reinstall` is the same without recreating the launcher.

---

## First run: `nabu --setup`

`--setup` is where the heavy, one-time work happens. `bootstrap.rs` extracts the
embedded `uv` + `transcribe.tar.gz` into `~/.nabu/`, then runs
`uv run --directory ~/.nabu/transcribe python transcribe.py --setup`. That:

1. **Provisions Python.** `uv` reads `transcribe/.python-version` (3.13) and
   `requires-python = ">=3.10,<3.14"`, then creates `~/.nabu/transcribe/.venv`.
   If no compatible interpreter is on the machine, `uv` downloads a managed one —
   this is what keeps setup working on a fresh macOS that only ships Python 3.14
   (which `torch` has no wheels for; see architecture.md's Python-version note).
2. **Installs the Python deps** from `uv.lock` (mlx-whisper, torch, pyannote,
   wespeaker, …) into that venv.
3. **Downloads the models** (~3 GB): the mlx-whisper model via `huggingface_hub`,
   and the wespeaker model via `wespeaker.load_model`. Pyannote is only fetched
   when `--hf-token` is supplied.

### Where the bytes land

| Path | Contents | Disposable? |
|---|---|---|
| `/usr/local/bin/nabu` | the binary | yes — re-downloadable |
| `~/.nabu/` | uv, extracted `transcribe/`, `.venv`, `.version` | **yes** — `rm -rf ~/.nabu && nabu --setup` = clean reinstall |
| `~/.cache/huggingface/` | mlx-whisper (+ optional pyannote) models | yes, but shared with other HF tools |
| `~/.wespeaker/english/` | wespeaker diarization model | yes |
| `~/nabu_data/` | **your recordings + transcripts** | **no — never auto-deleted, back this up** |

The `~/.nabu/` vs `~/nabu_data/` split is deliberate — see architecture.md
("Why two separate directories?").

---

## Updating

The binary is self-contained, so an update is just a new binary — re-run
`install.sh`, or drop a fresh `nabu-aarch64-apple-darwin` over `/usr/local/bin/nabu`.

On the first run after an update, `bootstrap.rs` sees `~/.nabu/.version` no longer
matches the binary's `CARGO_PKG_VERSION` and **re-extracts the bundled Python
scripts** (you'll see `extracting Python scripts …`). Crucially it **preserves
`.venv` and the downloaded models**, so there's nothing to reinstall and no need
to re-run `--setup`. Recordings in `~/nabu_data/` are never touched.

If the internals look stale: `rm -rf ~/.nabu && nabu --setup` rebuilds them from
scratch without touching recordings.

---

## Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/uninstall.sh | bash
# or from a checkout:  make uninstall   (→ ./uninstall.sh)
```

Removes `/usr/local/bin/nabu`, `~/Applications/nabu.app`, `~/.nabu/`, and
`~/.wespeaker/`. It asks **separately** before deleting your recordings in
`~/nabu_data/` or the HuggingFace model cache (which may be shared with other
tools). macOS Privacy permissions are left in place — remove those by hand in
System Settings if you want a truly clean slate.

---

## Related Makefile targets

| Target | Purpose |
|---|---|
| `make build` | release binary → `bin/nabu-aarch64-apple-darwin` |
| `make install` / `make reinstall` | build + install locally (see above) |
| `make publish` | upload binary + wespeaker model to the `v<version>` release |
| `make publish-binary` | binary only |
| `make mirror-wespeaker` | tar the locally-cached wespeaker model and upload it (run `nabu --setup` first to populate the cache) |
| `make uninstall` | run `uninstall.sh` |
| `make clean` | `cargo clean` + wipe `~/.nabu`, `~/.wespeaker`, `.tmp/`, and the dev `.venv` |
