#!/usr/bin/env python3
"""
gen_voice.py - synthesize spoken lines with the macOS `say` command and save
as OGG/Vorbis into assets/audio/voice/.

Run from the repo root (macOS only — needs the `say` binary):
    .venv/bin/python tools/gen_voice.py

`say` only writes AIFF (there is no built-in Vorbis encoder on macOS), so each
line goes: `say` -> temp .aiff -> read with `soundfile` -> re-written as OGG
through the same soundfile/libsndfile path `gen_audio.py` uses for its
synthesized effects. That keeps every file under assets/audio/ one format,
and Bevy only has to know how to decode Vorbis.

Voice: the brief asked for Scottish (e.g. "Fiona"), which is not installed on
this machine — `say -v '?'` lists what's on it. This defaults to Daniel
(en_GB) as a stand-in. Once a Scottish voice is installed, regenerate with:

    SHOOTY_VOICE=Fiona .venv/bin/python tools/gen_voice.py

which re-synthesizes every line — there's no per-voice state to migrate.
"""

from __future__ import annotations

import os
import subprocess
import tempfile

import soundfile as sf

from gen_audio import OUT_DIR as SFX_DIR  # noqa: E402  (script-local import)

OUT_DIR = os.path.join(SFX_DIR, "voice")

VOICE = os.environ.get("SHOOTY_VOICE", "Daniel")
# Words per minute; unset lets `say` use the voice's own default pace.
RATE = os.environ.get("SHOOTY_VOICE_RATE")

# Keyed by output filename (see `audio.rs` for how each is loaded/wired):
#   voice_dual_guitar / voice_encore / voice_arpeggio  — pickup::PickupKind
#   voice_speaker / voice_stage                        — build::StructureKind
#   voice_cured_<stem>                                 — one per gloom::STEMS
#   voice_all_cured                                    — every stem cured
LINES = {
    "voice_dual_guitar.ogg": "Dual guitar!",
    "voice_encore.ogg": "Encore's loaded!",
    "voice_arpeggio.ogg": "Arpeggio!",
    "voice_speaker.ogg": "Speaker's up!",
    "voice_stage.ogg": "Stage is raised!",
    "voice_cured_hiphop.ogg": "The hip-hop head's back on side!",
    "voice_cured_techno.ogg": "The techno kid's back on side!",
    "voice_cured_disco.ogg": "The disco dancer's back on side!",
    "voice_cured_country.ogg": "The country singer's back on side!",
    "voice_cured_jpop.ogg": "The J-pop star's back on side!",
    "voice_all_cured.ogg": "The whole crowd's back!",
}


def say_to_aiff(text: str, path: str) -> None:
    cmd = ["say", "-v", VOICE]
    if RATE:
        cmd += ["-r", RATE]
    cmd += ["-o", path, text]
    subprocess.run(cmd, check=True)


def main() -> None:
    os.makedirs(OUT_DIR, exist_ok=True)
    print(f"voice: {VOICE}" + (f"  rate: {RATE}" if RATE else ""))
    print(f"writing to {OUT_DIR}")
    with tempfile.TemporaryDirectory() as tmp:
        aiff = os.path.join(tmp, "line.aiff")
        for name, text in LINES.items():
            say_to_aiff(text, aiff)
            data, sr = sf.read(aiff, dtype="float32")
            out_path = os.path.join(OUT_DIR, name)
            sf.write(out_path, data, sr, format="OGG", subtype="VORBIS")
            info = sf.info(out_path)
            size = os.path.getsize(out_path)
            print(f'  {name:28s} {info.duration:5.2f}s  {size / 1024:6.1f} KB  "{text}"')


if __name__ == "__main__":
    main()
