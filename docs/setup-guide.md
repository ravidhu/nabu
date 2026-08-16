# nabu — Setup Guide

This guide walks you through installing nabu from scratch. No technical background required.

**What nabu does:** records your microphone and system audio (calls, speakers) simultaneously, then automatically transcribes everything into a text file — with speaker labels.

**What you need:** a Mac with Apple Silicon (M1/M2/M3/M4) running macOS 13 or later.

---

## Overview

There are four one-time setup steps:

1. Get the nabu binary
2. Download the AI models
3. Grant macOS permissions
4. Start recording

After setup, using nabu is just: open Terminal → type `nabu` → press Enter.

> **If anything goes wrong at any point, run `nabu --doctor`.** It checks macOS version, permissions, model downloads, disk space, and tells you exactly what to fix.

---

## Step 1 — Get nabu

### Option A — One-line installer (recommended)

Open Terminal (press `Cmd + Space`, type "Terminal", press Enter) and paste:

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/install.sh | bash
```

This downloads the binary, installs it, downloads the AI models (~3 GB), creates a **nabu.app** launcher in `~/Applications` that you can drag to your Dock, and finishes by running `nabu --doctor` to tell you what (if anything) still needs your attention.

If everything is green in the doctor output, **skip Steps 2 and 3 below** and jump to **[Step 4 — Start recording](#youre-ready)**. The installer covers Steps 1 and 2 in full, and Step 4 is just `nabu`.

If the doctor reports failures — almost always **Screen Recording** and possibly **Microphone** — read Step 3 below: it walks you through granting them. macOS does not let an installer grant permissions on your behalf, so this step has to be done in System Settings. You can re-run `nabu --doctor` at any time to re-check.

---

### Option B — Pre-built binary (manual)

**If you cloned the repo locally**, build the binary then copy it directly:

```bash
make build
sudo cp bin/nabu-aarch64-apple-darwin /usr/local/bin/nabu
```

**If you downloaded the file from the [latest GitHub Release](https://github.com/ravidhu/nabu/releases/latest)** via a browser or `curl`, macOS adds a quarantine flag that blocks it. Remove it first:

```bash
curl -fLo ~/Downloads/nabu-aarch64-apple-darwin \
  https://github.com/ravidhu/nabu/releases/latest/download/nabu-aarch64-apple-darwin
chmod +x ~/Downloads/nabu-aarch64-apple-darwin
sudo mv ~/Downloads/nabu-aarch64-apple-darwin /usr/local/bin/nabu
sudo xattr -d com.apple.quarantine /usr/local/bin/nabu
```

> macOS only sets the quarantine flag on files downloaded from the internet. If you copied from a local clone, the `xattr` step is not needed and will print "No such xattr" — that's fine.

Confirm it works:

```bash
nabu --version
```

You should see `nabu 0.1.0`.

---

### Option C — Build from source

If you prefer to compile nabu yourself (or Option A doesn't work on your Mac):

1. **Install Rust** — paste this in Terminal:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Press **Enter** to accept defaults. When done, close and reopen Terminal.

2. **Download the nabu source** — if you have git:

```bash
git clone https://github.com/ravidhu/nabu.git
cd nabu
```

Otherwise download the zip from GitHub, unzip it, right-click the folder → **New Terminal at Folder**.

3. **Build:**

```bash
cargo build --release
```

Takes 2–5 minutes the first time — you'll see a lot of output, that's normal. Wait for `Finished release`.

4. **Install:**

```bash
sudo cp target/release/nabu /usr/local/bin/nabu
```

> nabu's binary includes everything it needs — no Python or separate tools required.

---

## Step 2 — Download the AI models

nabu needs to download AI models before it can transcribe. This happens once and then works fully offline. Most users only need **Option A** below — Option B is for anyone who wants a sharper version of the "who spoke when" labels and doesn't mind creating a free HuggingFace account.

### Option A: Standard setup (recommended)

This downloads the Whisper speech-to-text model (`large-v3`, ~3 GB) and the wespeaker speaker-identification model (~26 MB). Recordings will label your microphone as **[You]** and automatically identify other speakers as **[Speaker 1]**, **[Speaker 2]**, etc.

```bash
nabu --setup
```

You'll see download progress — first-time download can take 5–15 minutes depending on your internet speed. When it finishes, models are cached locally and nabu works fully offline from now on.

> Prefer a smaller download? Pass `--model distil-large-v3` (~1.5 GB) or `--model small.en` (~480 MB, English only). See `nabu --list-models` for the full list.

---

### Option B: Higher-accuracy speaker labels (optional)

If multi-speaker labelling matters a lot to you — for example for transcribing interviews or meetings with several remote participants — you can swap the default `wespeaker` model for `pyannote`, which is more accurate at telling speakers apart. The trade-off is a one-time setup that requires a free HuggingFace account.

| | Option A (default) | Option B |
|---|---|---|
| Speakers labelled | Yes | Yes — more accurate |
| Account needed | No | Yes — free HuggingFace account |
| Extra download | — | ~300 MB |

**Step-by-step for Option B:**

1. Create a free account at [huggingface.co/join](https://huggingface.co/join)

2. Accept the model terms (one-time, required by the model license):
   - Go to [pyannote/speaker-diarization-community-1](https://huggingface.co/pyannote/speaker-diarization-community-1) → click **Agree and access repository**
   - Go to [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0) → click **Agree and access repository**

3. Create an access token at [huggingface.co/settings/tokens](https://huggingface.co/settings/tokens):
   - Click **New token**
   - Give it any name (e.g. "nabu")
   - Select **Read** access
   - Click **Create token** and copy it (starts with `hf_`)

4. Run setup with your token:

```bash
nabu --setup --hf-token YOUR_TOKEN_HERE
```

Replace `YOUR_TOKEN_HERE` with the token you copied.

Once setup finishes, nabu is fully offline — you don't need the token again. To use this diarizer for a recording, pass `--diarizer pyannote`.

---

## Step 3 — Grant macOS permissions

nabu needs two permissions.

> **Why Terminal and not nabu?** macOS grants permissions to the app that launches a program, not the program itself. Because nabu runs inside your terminal, macOS sees Terminal (or iTerm2, Warp, etc.) as the app making the request. You grant the permission once to your terminal app — all CLI tools you run inside it inherit it automatically.

### 3a. Microphone

macOS will ask automatically the first time you run nabu. Click **Allow**.

If you missed it or accidentally denied it: nabu will detect the failure and open the right pane for you. Otherwise:
1. Open **System Settings**
2. Go to **Privacy & Security → Microphone**
3. Find your terminal app (Terminal, iTerm2, Warp…) and turn it **on**

---

### 3b. Screen Recording (required for system audio)

This lets nabu capture the audio playing through your speakers — calls, videos, meetings. Despite the name "Screen Recording", nabu **does not record your screen**. It only captures audio.

1. Open **System Settings**
2. Go to **Privacy & Security → Screen Recording**
3. Click the **+** button
4. Find and add your terminal app:
   - **Apple's Terminal** lives in `/System/Applications/Utilities/` (use `Shift + Cmd + G` in the file dialog and paste that path).
   - **iTerm2**, **Warp**, **Ghostty**, etc. live in `/Applications/`.

After adding it, **quit and reopen your terminal app** for the permission to take effect. macOS caches the permission decision until the terminal process restarts.

> Tip: run `nabu --doctor` to confirm everything is green before your first recording.

---

## Step 4 — Start recording

Open Terminal and run:

<a id="youre-ready"></a>

```bash
nabu
```

nabu starts recording immediately. You'll see a live timer:

```
● REC  00:01:23  —  Ctrl-C to stop
```

Press **Ctrl-C** to stop. Transcription runs automatically and the results are saved to `~/nabu_data/`. The session folder is opened in Finder when transcription finishes.

> **About long recordings:** nabu auto-stops after **4 hours** so a forgotten Ctrl-C cannot fill your disk overnight. In the last 10 minutes the timer asks `press [e] to extend +1h`. Press `e` to keep going for another hour. To raise or lower the cap, pass `--max-duration 8h` (or any `h`/`m`/`s` value like `90m`).

---

## Where are my recordings?

All recordings are saved in your home folder under `nabu_data/`:

```
~/nabu_data/
└── 2026_05_27_14_32/          ← folder named by date and time
    └── merged.wav              ← mic + system audio combined
    └── transcript.md           ← text transcript with speaker labels
```

To open a transcript, double-click it in Finder — it opens in TextEdit. Or in Terminal:

```bash
cat ~/nabu_data/2026_05_27_14_32/transcript.md
```

> Want to see *all* the locations nabu uses (binary, model caches, sessions)? Run `nabu --where`.

---

## Example transcript

With speaker labels (default):
```
[00:00:03 → 00:00:04] [You]       Hey, can you hear me?
[00:00:05 → 00:00:07] [Speaker 1] Yes loud and clear.
[00:00:09 → 00:00:11] [You]       Great, let's get started.
[00:00:14 → 00:00:17] [Speaker 2] Joining now, sorry for the delay.
```

Without speaker identification (if diarization failed):
```
[00:00:03 → 00:00:04] [You]     Hey, can you hear me?
[00:00:05 → 00:00:07] [Remote]  Yes loud and clear.
```

---

## Common options

**Record audio only (skip transcription):**
```bash
nabu --no-stt
```

**Use the higher-accuracy pyannote diarizer (requires Option B setup):**
```bash
nabu --diarizer pyannote
```

**Force a specific language (skip the interactive picker and auto-detect):**
```bash
nabu -l fr          # short form of --language
nabu --language fr
```

Without `-l`, a multilingual model prompts with a numbered language menu before transcription starts (Enter = auto-detect). Pass `-y` to suppress it in scripts.

**Cap the recording at 1 hour instead of 4:**
```bash
nabu --max-duration 1h
```

**Transcribe an audio file you already have:**
```bash
nabu --file ~/Downloads/meeting.wav
```

---

## Troubleshooting

> **Always try this first:** `nabu --doctor`. It runs nine checks — macOS version, Apple Silicon, microphone, screen recording, AI models, uv runtime, disk space — and prints exactly what to fix.

### "command not found: nabu"

The binary isn't on your `PATH`. Either run it directly from the project folder:

```bash
./target/release/nabu
```

Or re-do Step 1 to install it system-wide.

---

### "nabu is not set up yet — run 'nabu --setup' first"

Run the setup command:

```bash
nabu --setup
```

---

### Screen Recording permission keeps failing

After adding your terminal in *System Settings → Privacy & Security → Screen Recording*, **quit your terminal completely** (`Cmd + Q`, not just close the window) and reopen it. macOS caches the permission decision until the terminal process restarts.

If you're not sure which terminal app you're using, run:

```bash
echo "$TERM_PROGRAM"
```

---

### nabu shows the recording timer but transcript is empty

Check that system audio is actually playing through your Mac's speakers during recording. If audio is routed to a USB-C hub or some Bluetooth setups, it may bypass ScreenCaptureKit. Re-run with audio playing through the Mac's built-in output.

---

### Transcription is slow

`large-v3` (the default) is the most accurate but also the slowest. Use a smaller model:

```bash
nabu --model small.en        # English only, ~5× faster
nabu --model distil-large-v3 # multilingual, ~2× faster
```

See `nabu --list-models` for everything available.

---

### The recording folder is in `.tmp/`

If nabu was force-quit (killed, not Ctrl-C), the session folder stays in `~/nabu_data/.tmp/`. Your audio files are still intact there — just move them out manually:

```bash
mv ~/nabu_data/.tmp/2026_05_27_14_32 ~/nabu_data/
```

---

### No speaker labels — everyone is "[Remote]"

This means diarization failed or was skipped. Common causes:
- The system audio was silent during recording (no one spoke through the speakers).
- The recording was very short (under ~10 seconds).

Speaker labels work best on recordings of 30 seconds or more with clear audio.

---

### Build errors during `cargo build --release`

**"error: linker `cc` not found"** — install Xcode Command Line Tools:
```bash
xcode-select --install
```

**"could not compile `screencapturekit`"** — requires macOS 13+. Check your macOS version in *System Settings → General → About*.

---

## Updating nabu

**If you used the installer (Option A):** re-run it. It overwrites the binary in place.

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/install.sh | bash
```

**If you built from source (Option C):**

```bash
cd nabu
git pull
cargo build --release
sudo cp target/release/nabu /usr/local/bin/nabu
```

The setup step (`nabu --setup`) does not need to be repeated unless the models change.

---

## Uninstalling nabu

To remove nabu and all of its caches:

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/uninstall.sh | bash
```

Or, from a checkout:

```bash
make uninstall
```

It removes the binary, the `nabu.app` launcher, `~/.nabu/`, and `~/.wespeaker/`. It asks you separately before deleting your recordings in `~/nabu_data/` or the HuggingFace model cache (which may be shared with other tools). macOS Privacy permissions stay until you remove them by hand in *System Settings → Privacy & Security*.
