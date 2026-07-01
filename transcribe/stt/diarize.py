"""Speaker diarization backends: wespeaker (default) and pyannote.

Both return the same shape: ``list[{"start": float, "end": float, "speaker": str}]``,
which :func:`stt.transcript.assign_speakers` uses to label segments.

Heavy: torch / pyannote / wespeaker are imported lazily inside the functions.
"""
from __future__ import annotations

import contextlib
import io
import os
from pathlib import Path

from .models import PYANNOTE_MODEL


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
