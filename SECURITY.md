# Security policy

## Scope

nabu is a local-only macOS recording and transcription tool. It runs entirely on the user's machine and reaches the network only during `nabu --setup`, to download AI models from HuggingFace. After setup, nabu operates fully offline.

In scope for security reports:

- Any code path in this repository that contacts the network outside of `nabu --setup`.
- Any code path that writes outside of `~/.nabu/`, `~/.wespeaker/`, or the configured session directory.
- Anything that would cause recordings, transcripts, or the HuggingFace token to leak off-device.
- Privilege escalation, arbitrary code execution, or unsafe extraction of the embedded Python pipeline / `uv` binary.
- Memory safety issues in the Rust code (`unsafe` blocks, FFI to ScreenCaptureKit / CoreGraphics).

Out of scope:

- Vulnerabilities in upstream dependencies (`mlx-whisper`, `pyannote`, `wespeaker`, `cpal`, `screencapturekit`, `uv`). Please report those to the respective projects.
- Issues that require an attacker to already have local code execution as the user running nabu.
- Issues in third-party tools used during development (`cargo`, `rustup`, `uv`).

## Supported versions

Only the latest release is supported. There are no backported fixes for older versions.

## Reporting a vulnerability

Please report security issues privately by email to **ravidhu.dissa@gmail.com**. Include:

- A description of the issue and its impact.
- Steps to reproduce or a proof of concept.
- The nabu version (`nabu --version`) and macOS version.

You will receive an acknowledgement within 7 days. nabu follows a **90-day disclosure window**: the issue will be disclosed publicly no later than 90 days after the initial report, regardless of fix availability, unless we mutually agree on a different timeline.

Please do not open GitHub issues or discussion threads for security reports.
