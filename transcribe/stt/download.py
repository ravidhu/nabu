"""Model pre-download / caching for `nabu --setup`.

Heavy: huggingface-hub / wespeaker / pyannote are imported lazily inside
:func:`cmd_setup` so importing this module stays cheap.
"""
from __future__ import annotations

import contextlib
import io
from pathlib import Path

from .models import PYANNOTE_MODEL, WESPEAKER_MIRROR_URL, mlx_repo


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


def cmd_setup(model_name: str, hf_token: str | None) -> None:
    """Pre-download all models so subsequent runs are fully offline."""
    from huggingface_hub import snapshot_download

    # mlx-whisper — public, no token
    repo = mlx_repo(model_name)
    print(f"[nabu setup] downloading mlx-whisper model '{model_name}' ({repo}) …", flush=True)
    snapshot_download(repo_id=repo)
    print(f"[nabu setup] ✓ mlx-whisper {model_name}", flush=True)

    # wespeaker — download from GitHub mirror, fall back to ModelScope
    print(f"[nabu setup] downloading wespeaker speaker model ({WESPEAKER_MIRROR_URL}) …", flush=True)
    try:
        _ensure_wespeaker_cached()
    except Exception:
        # mirror failed — let wespeaker fall back to its ModelScope CDN
        print("[nabu setup]   mirror unavailable — falling back to ModelScope (modelscope.cn) …", flush=True)
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
