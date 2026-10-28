#!/usr/bin/env python3
"""Create the Letter Flip voice pack with the installed Mac Jamie (Premium) voice.

The source text is voice-lines.tsv. Output is always the adjacent mp3 directory.
Use --check to verify the pack, or --force to replace files for these known names.
No other voice is used if Jamie (Premium) is not installed.
"""
from __future__ import annotations

import argparse
import array
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent
CATALOG = ROOT / "voice-lines.tsv"
OUTPUT = ROOT / "mp3"
VOICE = "Jamie (Premium)"
RATE = 155
SAMPLE_RATE = 22050
BIT_RATE = "64k"
MAX_SECONDS = 29.0
MANIFEST = "generation.json"


class GenerationError(RuntimeError):
    """A required tool, voice, source, or audio operation failed."""


def load_catalog(path: Path) -> dict[str, str]:
    clips: dict[str, str] = {}
    lines = path.read_text(encoding="utf-8").splitlines()
    for line_number, line in enumerate(lines, 1):
        if not line or line.startswith("#") or line == "name\ttext":
            continue
        parts = line.split("\t")
        if len(parts) != 2:
            raise GenerationError(f"{path}:{line_number}: use one name and one text field")
        name, text = parts
        if not re.fullmatch(r"[a-z][a-z0-9_]*", name) or not text.strip():
            raise GenerationError(f"{path}:{line_number}: invalid name or empty text")
        if name in clips:
            raise GenerationError(f"{path}:{line_number}: duplicate name {name}")
        clips[name] = text.strip()
    required = set("abcdefghijklmnopqrstuvwxyz") | {
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine"
    }
    missing = required - clips.keys()
    if missing:
        raise GenerationError(f"The catalog is missing: {', '.join(sorted(missing))}")
    return clips


def run(command: list[str], *, timeout: int = 60) -> bytes:
    try:
        result = subprocess.run(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=timeout, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise GenerationError(f"Could not run {command[0]}: {error}") from error
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise GenerationError(f"{Path(command[0]).name} failed ({result.returncode}): {detail}")
    return result.stdout


def tools() -> dict[str, str]:
    if platform.system() != "Darwin":
        raise GenerationError("Voice generation requires macOS with Jamie (Premium) installed")
    found: dict[str, str] = {}
    for name in ("say", "ffmpeg", "ffprobe"):
        executable = shutil.which(name)
        if not executable:
            raise GenerationError(f"Required tool is missing: {name}")
        found[name] = executable
    voices = run([found["say"], "-v", "?"]).decode("utf-8")
    installed = any(re.match(r"^" + re.escape(VOICE) + r"\s+en_GB\s+#", line)
                    for line in voices.splitlines())
    if not installed:
        raise GenerationError(f"{VOICE} is not installed. Install it in macOS Spoken Content settings")
    return found


def fingerprint(text: str) -> dict[str, object]:
    return {"voice": VOICE, "rate": RATE, "sample_rate": SAMPLE_RATE,
            "bit_rate": BIT_RATE, "filter": "loudnorm=I=-20:TP=-3:LRA=7,apad=pad_dur=0.08",
            "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest()}


def validate_audio(path: Path, executables: dict[str, str]) -> float:
    if not path.is_file() or path.stat().st_size == 0:
        raise GenerationError(f"Missing or empty MP3: {path}")
    probe = json.loads(run([executables["ffprobe"], "-v", "error", "-show_entries",
                           "stream=codec_name,sample_rate,channels:format=duration", "-of", "json", str(path)]))
    streams = probe.get("streams", [])
    if len(streams) != 1 or streams[0].get("codec_name") != "mp3" \
            or streams[0].get("channels") != 1 or streams[0].get("sample_rate") != str(SAMPLE_RATE):
        raise GenerationError(f"Unexpected audio format: {path}")
    try:
        duration = float(probe["format"]["duration"])
    except (KeyError, ValueError, TypeError) as error:
        raise GenerationError(f"Missing duration: {path}") from error
    if not math.isfinite(duration) or not 0.1 <= duration <= MAX_SECONDS:
        raise GenerationError(f"Audio duration is outside 0.1..{MAX_SECONDS} seconds: {path} ({duration})")
    # Decode the complete stream. Check actual sample data rather than file size alone.
    pcm = run([executables["ffmpeg"], "-hide_banner", "-loglevel", "error", "-xerror", "-nostdin",
               "-i", str(path), "-f", "s16le", "-ac", "1", "-ar", str(SAMPLE_RATE), "pipe:1"])
    if len(pcm) % 2:
        raise GenerationError(f"Invalid decoded sample length: {path}")
    samples = array.array("h")
    samples.frombytes(pcm)
    if sys.byteorder != "little":
        samples.byteswap()
    if not samples or max(abs(value) for value in samples) < 32:
        raise GenerationError(f"Audio is silent: {path}")
    return duration


def generate_clip(name: str, text: str, destination: Path,
                  executables: dict[str, str]) -> dict[str, object]:
    # Preparation and validation occur on the same filesystem as the final file.
    # A failed say, encode, or decode leaves the previous destination unchanged.
    with tempfile.TemporaryDirectory(prefix=".voice-", dir=destination.parent) as temporary:
        staging = Path(temporary)
        source = staging / "text.txt"
        source.write_text(text, encoding="utf-8")
        aiff = staging / "speech.aiff"
        mp3 = staging / f"{name}.mp3"
        run([executables["say"], "-v", VOICE, "-r", str(RATE), "-f", str(source), "-o", str(aiff)])
        run([executables["ffmpeg"], "-hide_banner", "-loglevel", "error", "-xerror", "-nostdin", "-y",
             "-i", str(aiff), "-map_metadata", "-1", "-af", str(fingerprint(text)["filter"]),
             "-codec:a", "libmp3lame", "-b:a", BIT_RATE, "-ar", str(SAMPLE_RATE), "-ac", "1", str(mp3)])
        duration = validate_audio(mp3, executables)
        metadata = {**fingerprint(text), "mp3_sha256": hashlib.sha256(mp3.read_bytes()).hexdigest(),
                    "duration_seconds": duration}
        with mp3.open("rb") as handle:
            os.fsync(handle.fileno())
        os.replace(mp3, destination)
        return metadata


def read_manifest(path: Path) -> dict[str, dict[str, object]]:
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise GenerationError(f"Could not read {path}: {error}") from error
    if not isinstance(data, dict) or any(not isinstance(value, dict) for value in data.values()):
        raise GenerationError(f"Invalid generation manifest: {path}")
    return data


def write_manifest(path: Path, metadata: dict[str, dict[str, object]]) -> None:
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", prefix=".generation-",
                                     dir=path.parent, delete=False) as handle:
        temporary = Path(handle.name)
        try:
            json.dump(metadata, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
            os.replace(temporary, path)
        finally:
            temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Verify all clips without changing files")
    parser.add_argument("--force", action="store_true", help="Replace all MP3s with names in the catalog")
    parser.add_argument("--list", action="store_true", help="Show names and text without generating audio")
    options = parser.parse_args()
    if options.check and options.force:
        parser.error("--check and --force cannot be used together")
    try:
        catalog = load_catalog(CATALOG)
        if options.list:
            for name, text in catalog.items():
                print(f"{name}.mp3\t{text}")
            return 0
        executables = tools()
        if options.check and not OUTPUT.is_dir():
            raise GenerationError(f"Voice folder does not exist: {OUTPUT}")
        if OUTPUT.is_symlink():
            raise GenerationError(f"The voice folder must be a real directory: {OUTPUT}")
        OUTPUT.mkdir(exist_ok=True)
        manifest_path = OUTPUT / MANIFEST
        metadata = read_manifest(manifest_path)
        for index, (name, text) in enumerate(catalog.items(), 1):
            destination = OUTPUT / f"{name}.mp3"
            prior = metadata.get(name)
            same_source = prior is not None and all(prior.get(key) == value for key, value in fingerprint(text).items())
            same_bytes = destination.is_file() and prior is not None \
                and hashlib.sha256(destination.read_bytes()).hexdigest() == prior.get("mp3_sha256")
            if not options.force and same_source and same_bytes:
                validate_audio(destination, executables)
                print(f"[{index}/{len(catalog)}] Verified {destination.name}", flush=True)
                continue
            if options.check:
                raise GenerationError(f"Missing, changed, or out-of-date clip: {destination}. Run this script to create the pack")
            if destination.exists() and prior is not None and not same_bytes and not options.force:
                raise GenerationError(f"Existing clip changed since generation: {destination}. Use --force only to replace it deliberately")
            if destination.exists() and prior is None and not options.force:
                raise GenerationError(f"Existing clip has no generation record: {destination}. Use --force only to replace it deliberately")
            print(f"[{index}/{len(catalog)}] Generate {destination.name}: {text}", flush=True)
            metadata[name] = generate_clip(name, text, destination, executables)
            write_manifest(manifest_path, metadata)
        print(f"Verified all {len(catalog)} Jamie (Premium) MP3s in {OUTPUT}")
        return 0
    except (GenerationError, OSError, ValueError, KeyError) as error:
        print(f"Voice generation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
