"""Speech-to-text via mlx-whisper (Metal GPU).

Heavy: imports ``mlx_whisper`` lazily inside :func:`transcribe` so importing this
module (or anything that imports it) stays cheap.
"""
from __future__ import annotations

from pathlib import Path

from .cleanup import filter_hallucinations
from .models import mlx_repo


def transcribe(audio_path: Path, model_name: str, language: str | None = None) -> list[dict]:
    """Transcribe audio with mlx-whisper. Returns segment dicts with timestamps."""
    import mlx_whisper

    repo = mlx_repo(model_name)
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
                # Decoder confidence signals — used by the hallucination filter,
                # then stripped before returning (downstream wants start/end/text).
                "no_speech_prob":    seg.get("no_speech_prob"),
                "avg_logprob":       seg.get("avg_logprob"),
                "compression_ratio": seg.get("compression_ratio"),
            })

    kept = filter_hallucinations(segments)
    dropped = len(segments) - len(kept)
    if dropped:
        print(f"[nabu] filtered {dropped} likely-hallucinated segment(s) from {audio_path.name}", flush=True)
    return [{"start": s["start"], "end": s["end"], "text": s["text"]} for s in kept]
