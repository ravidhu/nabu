"""
nabu transcription — mlx-whisper (Metal GPU) + speaker diarization.

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
import contextlib
import io
import os
import sys
import warnings
from pathlib import Path

# Suppress noisy warnings from pyannote and torchaudio about missing FFmpeg/torchcodec.
# We bypass torchaudio for our own audio loading (soundfile), but sub-libraries still
# emit these warnings on import. Filter by module so the leading \n in the message
# doesn't break the regex match.
warnings.filterwarnings("ignore", category=UserWarning, module="pyannote.audio.core.io")
warnings.filterwarnings("ignore", category=UserWarning, module="torchaudio._backend")
warnings.filterwarnings("ignore", category=UserWarning, module="torchaudio._internal")
warnings.filterwarnings("ignore", category=UserWarning, module="s3prl")


# ── model registry ────────────────────────────────────────────────────────────

PYANNOTE_MODEL = "pyannote/speaker-diarization-community-1"

# GitHub Releases mirror for the wespeaker English model — avoids the slow
# ModelScope CDN (modelscope.cn). wespeaker checks ~/.wespeaker/english/ first,
# so pre-populating it bypasses the ModelScope download entirely.
WESPEAKER_MIRROR_URL = (
    "https://raw.githubusercontent.com/ravidhu/nabu/main/models/wespeaker/"
    "voxceleb_resnet221_LM.tar.gz"
)

# Maps Whisper shorthand names → mlx-community HuggingFace repos
MLX_REPOS: dict[str, str] = {
    "tiny.en":         "mlx-community/whisper-tiny.en-mlx",
    "tiny":            "mlx-community/whisper-tiny-mlx",
    "base.en":         "mlx-community/whisper-base.en-mlx",
    "base":            "mlx-community/whisper-base-mlx",
    "small.en":        "mlx-community/whisper-small.en-mlx",
    "small":           "mlx-community/whisper-small-mlx",
    "medium.en":       "mlx-community/whisper-medium.en-mlx",
    "medium":          "mlx-community/whisper-medium-mlx",
    "large-v3":        "mlx-community/whisper-large-v3-mlx",
    "large":           "mlx-community/whisper-large-v3-mlx",
    "distil-large-v3": "mlx-community/distil-whisper-large-v3",
    "distil-small.en": "mlx-community/distil-whisper-small.en-mlx",
}

def _mlx_repo(model_name: str) -> str:
    return MLX_REPOS.get(model_name, f"mlx-community/whisper-{model_name}-mlx")


# ── wespeaker mirror download ─────────────────────────────────────────────────

def _ensure_wespeaker_cached() -> None:
    """Download the wespeaker English model from our GitHub mirror.

    Mirrors wespeaker's own extraction logic so that load_model("english")
    finds the files already in place and skips the ModelScope download.
    """
    import os
    import tarfile
    import tempfile
    import urllib.request

    wespeaker_home = Path(os.environ.get("WESPEAKER_HOME", Path.home() / ".wespeaker"))
    model_dir = wespeaker_home / "english"

    if model_dir.exists() and {"avg_model.pt", "config.yaml"}.issubset(os.listdir(model_dir)):
        return

    model_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.NamedTemporaryFile(suffix=".tar.gz", delete=False) as tmp:
        tmp_path = Path(tmp.name)

    try:
        urllib.request.urlretrieve(WESPEAKER_MIRROR_URL, tmp_path)
        with tarfile.open(tmp_path) as tf:
            for member in tf:
                name = os.path.basename(member.name)
                fileobj = tf.extractfile(member)
                if fileobj is not None:
                    (model_dir / name).write_bytes(fileobj.read())
    finally:
        tmp_path.unlink(missing_ok=True)


# ── setup (pre-download) ──────────────────────────────────────────────────────

def cmd_setup(model_name: str, hf_token: str | None) -> None:
    """Pre-download all models so subsequent runs are fully offline."""
    from huggingface_hub import snapshot_download

    # mlx-whisper — public, no token
    repo = _mlx_repo(model_name)
    print(f"[nabu setup] downloading mlx-whisper model '{model_name}' ({repo}) …", flush=True)
    snapshot_download(repo_id=repo)
    print(f"[nabu setup] ✓ mlx-whisper {model_name}", flush=True)

    # wespeaker — download from GitHub mirror, fall back to ModelScope
    print("[nabu setup] downloading wespeaker speaker model …", flush=True)
    try:
        _ensure_wespeaker_cached()
    except Exception:
        pass  # mirror failed — let wespeaker fall back to ModelScope
    with contextlib.redirect_stdout(io.StringIO()):
        import wespeaker
        wespeaker.load_model("english")
    print("[nabu setup] ✓ wespeaker", flush=True)

    # pyannote — optional, gated model, requires HF token
    if hf_token:
        print(f"[nabu setup] downloading pyannote {PYANNOTE_MODEL} …", flush=True)
        from pyannote.audio import Pipeline
        Pipeline.from_pretrained(PYANNOTE_MODEL, token=hf_token)
        print("[nabu setup] ✓ pyannote diarization model", flush=True)
    else:
        print(
            "[nabu setup] skipping pyannote (no --hf-token provided).\n"
            "             Run 'nabu --setup --hf-token <TOKEN>' to also cache it.",
            flush=True,
        )

    print("[nabu setup] done — models cached, all future runs are offline.", flush=True)


# ── transcription ─────────────────────────────────────────────────────────────

def transcribe(audio_path: Path, model_name: str, language: str | None = None) -> list[dict]:
    """Transcribe audio with mlx-whisper. Returns segment dicts with timestamps."""
    import mlx_whisper

    repo = _mlx_repo(model_name)
    lang_label = language or "auto"
    print(f"[nabu] transcribing {audio_path.name} (mlx-whisper {model_name}, lang={lang_label}) …", flush=True)

    result = mlx_whisper.transcribe(
        str(audio_path),
        path_or_hf_repo=repo,
        word_timestamps=True,
        language=language,
        verbose=False,
    )

    segments = []
    for seg in result.get("segments", []):
        text = seg.get("text", "").strip()
        if text:
            segments.append({
                "start": seg["start"],
                "end":   seg["end"],
                "text":  text,
            })
    return segments


# ── diarization ───────────────────────────────────────────────────────────────
#
# Both backends return the same type: list[{"start": float, "end": float, "speaker": str}]
# assign_speakers() uses that list to label Whisper segments.

def diarize_wespeaker(audio_path: Path) -> list[dict]:
    """Diarize with wespeaker — no token, no account, models auto-download (~26 MB)."""
    print(f"[nabu] running speaker diarization on {audio_path.name} (wespeaker) …", flush=True)

    with contextlib.redirect_stdout(io.StringIO()):
        import wespeaker
        model = wespeaker.load_model("english")

    # Returns [(utt, begin_sec, end_sec, speaker_int), ...] or []
    result = model.diarize(str(audio_path))
    return [
        {"start": begin, "end": end, "speaker": f"Speaker {int(label) + 1}"}
        for (_, begin, end, label) in result
    ]


def _pipeline_from_cache(model_id: str) -> object:
    """Load a pyannote Pipeline from local cache only — no network, no token."""
    from pyannote.audio import Pipeline
    prev = os.environ.get("HF_HUB_OFFLINE")
    os.environ["HF_HUB_OFFLINE"] = "1"
    try:
        return Pipeline.from_pretrained(model_id)
    finally:
        if prev is None:
            os.environ.pop("HF_HUB_OFFLINE", None)
        else:
            os.environ["HF_HUB_OFFLINE"] = prev


def diarize_pyannote(audio_path: Path, hf_token: str | None) -> list[dict]:
    """Diarize with pyannote — higher accuracy, requires one-time setup with HF token."""
    import soundfile as sf
    import torch
    from pyannote.audio import Pipeline

    print(f"[nabu] running speaker diarization on {audio_path.name} (pyannote) …", flush=True)

    try:
        pipeline = _pipeline_from_cache(PYANNOTE_MODEL)
    except Exception:
        if not hf_token:
            raise RuntimeError(
                "pyannote model not cached. Run 'nabu --setup --hf-token <TOKEN>' once to download it."
            )
        pipeline = Pipeline.from_pretrained(PYANNOTE_MODEL, token=hf_token)

    if torch.backends.mps.is_available():
        try:
            pipeline.to(torch.device("mps"))
        except Exception:
            pass

    data, sample_rate = sf.read(str(audio_path), dtype="float32")
    waveform = torch.from_numpy(data).unsqueeze(0)  # (1, T) — mono
    result = pipeline({"waveform": waveform, "sample_rate": sample_rate})

    # pyannote >= 3.3 returns DiarizeOutput dataclass; older returns Annotation directly.
    annotation = result.speaker_diarization if hasattr(result, "speaker_diarization") else result
    return [
        {"start": turn.start, "end": turn.end, "speaker": speaker}
        for turn, _, speaker in annotation.itertracks(yield_label=True)
    ]


# Pure-Python post-processing lives in postprocess.py so it can be unit-tested
# without importing mlx-whisper/torch/pyannote.
from postprocess import assign_speakers, merge_and_write  # noqa: E402,F401


# ── entry point ───────────────────────────────────────────────────────────────

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

    merge_and_write(mic_segs, sys_segs, out_md, display_md)


if __name__ == "__main__":
    main()
