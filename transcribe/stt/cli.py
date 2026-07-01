"""Command-line entry point: argparse + the three run modes.

Modes
-----
Setup (pre-download models):
    python transcribe.py --setup [--model large-v3] [--hf-token TOKEN]

Transcribe a session:
    python transcribe.py <session_dir> [--model large-v3] [--diarizer wespeaker|pyannote]

Transcribe a single file:
    python transcribe.py --file audio.wav [--model large-v3] [--diarizer wespeaker|pyannote]
    Output is written to <audio>.md next to the input file.

Diarizers
---------
wespeaker  (default) No account or token needed. Downloads ~26 MB model automatically.
pyannote   Higher accuracy. Requires a one-time 'nabu --setup --hf-token TOKEN'.

Environment
-----------
HF_TOKEN   HuggingFace token for pyannote diarization (optional).

Transcript format
-----------------
[HH:MM:SS → HH:MM:SS] [You]       your mic
[HH:MM:SS → HH:MM:SS] [Speaker 1] remote speaker (with diarization)
[HH:MM:SS → HH:MM:SS] [Remote]    remote audio  (diarization failed / disabled)
"""
from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from .cleanup import dedup_cross_track
from .diarize import diarize_pyannote, diarize_wespeaker
from .download import cmd_setup
from .asr import transcribe
from .transcript import assign_speakers, merge_and_write


def main() -> None:
    parser = argparse.ArgumentParser(
        description="nabu transcription: mlx-whisper + speaker diarization."
    )
    parser.add_argument(
        "session_dir", type=Path, nargs="?",
        help="nabu session directory (mic.wav, system.wav)",
    )
    parser.add_argument(
        "--setup", action="store_true",
        help="Pre-download models and exit. No session_dir required.",
    )
    parser.add_argument("--model", default="large-v3", help="Whisper model name")
    parser.add_argument(
        "--language",
        default=None,
        metavar="LANG",
        help="Language code to force (e.g. en, fr, ja). Default: auto-detect.",
    )
    parser.add_argument(
        "--diarizer",
        default="wespeaker",
        choices=["wespeaker", "pyannote"],
        help="Speaker diarization backend (default: wespeaker, no token needed)",
    )
    parser.add_argument(
        "--hf-token",
        default=os.environ.get("HF_TOKEN"),
        help="HuggingFace token — only required for --diarizer pyannote",
    )
    parser.add_argument(
        "--file",
        type=Path,
        default=None,
        metavar="AUDIO",
        help="Transcribe a single audio file. Output written to <file>.md next to the input.",
    )
    parser.add_argument(
        "--final-dir",
        type=Path,
        default=None,
        help="Final session directory (after .tmp rename) — used only for display",
    )
    args = parser.parse_args()

    # ── setup mode ────────────────────────────────────────────────────────────
    if args.setup:
        cmd_setup(args.model, args.hf_token)
        return

    # ── single-file mode ──────────────────────────────────────────────────────
    if args.file:
        audio = args.file.resolve()
        if not audio.exists():
            print(f"error: {audio} not found", file=sys.stderr)
            sys.exit(1)
        out_md = audio.with_suffix(".md")
        raw = transcribe(audio, args.model, args.language)
        if raw:
            try:
                if args.diarizer == "pyannote":
                    diarization = diarize_pyannote(audio, args.hf_token)
                else:
                    diarization = diarize_wespeaker(audio)
                segs = assign_speakers(raw, diarization)
            except Exception as e:
                print(
                    f"[nabu] diarization failed: {e}\n"
                    "       Labelling as [Speaker].",
                    flush=True,
                )
                segs = [{**s, "speaker": "Speaker"} for s in raw]
        else:
            segs = []
        merge_and_write(segs, [], out_md)
        return

    # ── session mode ──────────────────────────────────────────────────────────
    if not args.session_dir:
        parser.error("session_dir or --file is required unless --setup is passed")

    session: Path = args.session_dir.resolve()
    mic_wav = session / "mic.wav"
    sys_wav = session / "system.wav"
    out_md  = session / "transcript.md"
    display_md = (args.final_dir / "transcript.md") if args.final_dir else out_md

    if not mic_wav.exists():
        print(f"error: {mic_wav} not found", file=sys.stderr)
        sys.exit(1)

    # Mic: always [You]
    mic_segs = [
        {**s, "speaker": "You"}
        for s in transcribe(mic_wav, args.model, args.language)
    ]

    # System: diarize then label each Whisper segment with the matched speaker.
    # Falls back to [Remote] on any error (missing model, no token, etc.).
    sys_segs: list[dict] = []
    if sys_wav.exists():
        raw_sys = transcribe(sys_wav, args.model, args.language)
        if raw_sys:
            try:
                if args.diarizer == "pyannote":
                    diarization = diarize_pyannote(sys_wav, args.hf_token)
                else:
                    diarization = diarize_wespeaker(sys_wav)
                sys_segs = assign_speakers(raw_sys, diarization)
            except Exception as e:
                print(
                    f"[nabu] diarization failed: {e}\n"
                    "       Labelling system audio as [Remote].",
                    flush=True,
                )
                sys_segs = [{**s, "speaker": "Remote"} for s in raw_sys]

    # Drop mic segments that are just acoustic echo of the system audio (the
    # remote party bleeding into the mic when the user isn't on headphones).
    deduped = dedup_cross_track(mic_segs, sys_segs)
    if len(deduped) != len(mic_segs):
        print(f"[nabu] removed {len(mic_segs) - len(deduped)} echoed mic segment(s)", flush=True)
    mic_segs = deduped

    merge_and_write(mic_segs, sys_segs, out_md, display_md)
