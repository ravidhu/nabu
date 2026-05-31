"""Pure-Python transcript post-processing.

Kept separate from ``transcribe.py`` so unit tests can import these helpers
without pulling in mlx-whisper, torch, or pyannote.
"""
from __future__ import annotations

from pathlib import Path


def assign_speakers(segments: list[dict], diarization: list[dict]) -> list[dict]:
    """Label each Whisper segment with the speaker that overlaps it the most.

    Falls back to the label ``Remote`` when no diarization turn overlaps the
    segment at all. Speaker IDs from the diarizer are renumbered to friendly
    ``Speaker N`` labels in the order they first appear.
    """
    speaker_map: dict[str, str] = {}
    counter = 1

    def friendly(raw: str) -> str:
        nonlocal counter
        if raw not in speaker_map:
            speaker_map[raw] = f"Speaker {counter}"
            counter += 1
        return speaker_map[raw]

    result = []
    for seg in segments:
        best_speaker = "Remote"
        best_overlap = 0.0
        for turn in diarization:
            overlap = min(turn["end"], seg["end"]) - max(turn["start"], seg["start"])
            if overlap > best_overlap:
                best_overlap = overlap
                best_speaker = friendly(turn["speaker"])
        result.append({**seg, "speaker": best_speaker})
    return result


def fmt_timestamp(seconds: float) -> str:
    h = int(seconds // 3600)
    m = int((seconds % 3600) // 60)
    s = int(seconds % 60)
    return f"{h:02d}:{m:02d}:{s:02d}"


def render_transcript(mic_segs: list[dict], sys_segs: list[dict]) -> str:
    """Render the merged transcript body. Pure string in, pure string out."""
    all_segs = sorted(mic_segs + sys_segs, key=lambda s: s["start"])
    lines = [
        f"[{fmt_timestamp(s['start'])} → {fmt_timestamp(s['end'])}] [{s['speaker']}] {s['text']}"
        for s in all_segs
    ]
    return "\n".join(lines) + "\n"


def merge_and_write(
    mic_segs: list[dict],
    sys_segs: list[dict],
    out: Path,
    display_path: Path | None = None,
) -> None:
    out.write_text(render_transcript(mic_segs, sys_segs), encoding="utf-8")
    print(f"[nabu] transcript → {display_path or out}", flush=True)
