"""nabu transcription — mlx-whisper (Metal GPU) + speaker diarization.

The entry point is :func:`stt.cli.main`; the top-level
``transcribe.py`` shim calls it so the Rust bootstrap can keep invoking
``python transcribe.py`` unchanged.

Layout
------
Pure (stdlib only — safe to import in tests without the heavy ML deps):
    models       Whisper shorthand → HF repo registry and backend model IDs
    cleanup      hallucination filter + cross-track echo dedup
    transcript   speaker labelling, timestamp formatting, transcript rendering

Heavy (import mlx-whisper / torch / pyannote / wespeaker lazily, inside functions):
    asr          mlx-whisper transcription
    diarize      wespeaker / pyannote diarization backends
    download     model pre-download / caching (`nabu --setup`)
    cli          argparse entry point wiring it all together
"""
from __future__ import annotations

import os
import warnings

# mlx-whisper's word-timestamp code imports numba during transcription, which launches
# numba's thread pool with the default thread count. wespeaker's umap_clusterer then
# sets NUMBA_NUM_THREADS=1 at import, and numba raises "Cannot set NUMBA_NUM_THREADS
# once the threads have been launched". Pin it to 1 here, before any heavy import, so
# both sides agree. (numba is only used for whisper's DTW alignment — single-threaded
# anyway — and umap, which wespeaker wants at 1 thread.)
os.environ["NUMBA_NUM_THREADS"] = "1"

# Suppress noisy warnings from pyannote and torchaudio about missing FFmpeg/torchcodec.
# We bypass torchaudio for our own audio loading (soundfile), but sub-libraries still
# emit these warnings on import. Filter by module so the leading \n in the message
# doesn't break the regex match. Done here so it runs before any backend import,
# whatever the entry path.
warnings.filterwarnings("ignore", category=UserWarning, module="pyannote.audio.core.io")
warnings.filterwarnings("ignore", category=UserWarning, module="torchaudio._backend")
warnings.filterwarnings("ignore", category=UserWarning, module="torchaudio._internal")
warnings.filterwarnings("ignore", category=UserWarning, module="s3prl")
# s3prl 0.4.18 has non-raw regex strings (`"\.(.+)"`) that emit a compile-time
# SyntaxWarning on first import. Third-party bug, cosmetic. Compile-time escape
# warnings are attributed to the *importing* frame, not s3prl, so match by
# message rather than module.
warnings.filterwarnings("ignore", category=SyntaxWarning, message="invalid escape sequence")
