#!/usr/bin/env python3
"""
gen_audio.py - synthesize placeholder audio stems for `shooty`.

Run from the repo root:
    .venv/bin/python tools/gen_audio.py

Pure-numpy synthesis. Writes 6 OGG/Vorbis files into assets/audio/:
    count_in.ogg   drumstick "1-2-3-4" count-in
    chord_stab.ogg big distorted power-chord payoff hit
    beat_click.ogg soft unobtrusive metronome tick
    build.ogg      mechanical "structure goes up" sound
    scrap.ogg      tiny bright salvage pickup blip
    music_loop.ogg seamlessly loopable bass + drums groove (~124 BPM)

Deterministic (all noise seeded) and idempotent (overwrites).

Encode path: numpy float32 -> soundfile.write(..., format='OGG', subtype='VORBIS').
(ffmpeg is not installed on this machine; libsndfile 1.2.2 bundled with the
`soundfile` wheel handles Vorbis. `soundfile` was pip-installed into .venv from
pypi.org - it is not on the default artifactory index.)
"""

from __future__ import annotations

import os

import numpy as np
import soundfile as sf

SR = 44100
BPM = 124.0
BEAT = 60.0 / BPM  # seconds per quarter note (~0.4839s)

OUT_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "audio")


# ----------------------------------------------------------------------------
# helpers
# ----------------------------------------------------------------------------
def t(n):
    return np.arange(n) / SR


def env_ad(n, attack, decay, curve=3.0):
    """Attack-decay envelope, exponential-ish decay."""
    e = np.ones(n)
    a = max(1, int(attack * SR))
    if a > 1:
        e[:a] = np.linspace(0.0, 1.0, a)
    d = np.arange(n)
    e *= np.exp(-curve * np.clip((d - a) / max(1, decay * SR), 0, None))
    return e


def one_pole_lp(x, cutoff):
    dt = 1.0 / SR
    rc = 1.0 / (2 * np.pi * cutoff)
    a = dt / (rc + dt)
    y = np.empty_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += a * (x[i] - acc)
        y[i] = acc
    return y


def one_pole_hp(x, cutoff):
    return x - one_pole_lp(x, cutoff)


def sat(x, drive):
    return np.tanh(drive * x)


def normalize(x, peak_dbfs=-3.0):
    peak = np.max(np.abs(x))
    if peak < 1e-9:
        return x
    target = 10.0 ** (peak_dbfs / 20.0)
    return x * (target / peak)


def dbfs(x):
    peak = float(np.max(np.abs(x)))
    rms = float(np.sqrt(np.mean(x ** 2)))
    pd = 20 * np.log10(peak) if peak > 0 else float("-inf")
    rd = 20 * np.log10(rms) if rms > 0 else float("-inf")
    return pd, rd


def write_ogg(name, data):
    data = np.asarray(data, dtype=np.float32)
    data = np.clip(data, -1.0, 1.0)
    path = os.path.join(OUT_DIR, name)
    sf.write(path, data, SR, format="OGG", subtype="VORBIS")
    info = sf.info(path)
    pd, rd = dbfs(data)
    size = os.path.getsize(path)
    print(f"  {name:16s} {info.duration:5.2f}s  peak {pd:6.1f} dBFS  rms {rd:6.1f} dBFS  {size/1024:6.1f} KB")
    return path


# ----------------------------------------------------------------------------
# instrument voices
# ----------------------------------------------------------------------------
def stick_click(rng, dur=0.05, accent=1.0, bright=2100.0):
    n = int(dur * SR)
    noise = rng.standard_normal(n)
    click = one_pole_hp(noise, 1400.0)
    # short resonant ping on top for the "tick" pitch
    ping = np.sin(2 * np.pi * bright * t(n)) * np.exp(-70 * t(n))
    e = env_ad(n, 0.0004, 0.012, curve=6.0)
    sig = (0.7 * click + 0.5 * ping) * e
    return sig * accent


def kick(dur=0.28):
    n = int(dur * SR)
    tt = t(n)
    f = 120.0 * np.exp(-tt / 0.03) + 45.0
    phase = 2 * np.pi * np.cumsum(f) / SR
    body = np.sin(phase) * np.exp(-tt / 0.09)
    click = one_pole_hp(np.sin(2 * np.pi * 1600 * tt), 800.0) * np.exp(-tt / 0.004) * 0.3
    return body + click


def snare(rng, dur=0.2):
    n = int(dur * SR)
    tt = t(n)
    noise = one_pole_hp(rng.standard_normal(n), 1200.0)
    tone = (np.sin(2 * np.pi * 185 * tt) + 0.6 * np.sin(2 * np.pi * 278 * tt))
    e_n = np.exp(-tt / 0.09)
    e_t = np.exp(-tt / 0.05)
    return 0.8 * noise * e_n + 0.35 * tone * e_t


def hat(rng, dur=0.06, open_=False):
    n = int(dur * SR)
    tt = t(n)
    noise = one_pole_hp(rng.standard_normal(n), 6500.0)
    tau = 0.04 if open_ else 0.018
    return noise * np.exp(-tt / tau)


def bass_note(freq, dur, gain=1.0):
    n = int(dur * SR)
    tt = t(n)
    # slightly detuned saw-ish stack -> filtered -> pluck env
    saw = np.zeros(n)
    for k in range(1, 12):
        saw += np.sin(2 * np.pi * freq * k * tt) / k
    saw += 0.5 * np.sin(2 * np.pi * freq * 0.999 * tt)
    cutoff = 380.0 + 900.0 * np.exp(-tt / 0.08)
    # time-varying-ish lowpass approximated with a fixed pass then env
    filt = one_pole_lp(saw, 700.0)
    e = env_ad(n, 0.005, dur * 0.7, curve=2.2)
    return filt * e * gain


# ----------------------------------------------------------------------------
# stems
# ----------------------------------------------------------------------------
def make_count_in():
    rng = np.random.default_rng(1001)
    spacing = BEAT  # ~0.484s at 124 BPM
    total = int((spacing * 3 + 0.35) * SR)
    buf = np.zeros(total)
    for i in range(4):
        accent = 1.15 if i == 3 else 1.0
        c = stick_click(rng, dur=0.06, accent=accent, bright=2050.0 + 40 * i)
        start = int(i * spacing * SR)
        buf[start:start + len(c)] += c[: max(0, total - start)]
    return normalize(buf, -3.0)


def make_chord_stab():
    dur = 1.5
    n = int(dur * SR)
    tt = t(n)
    root = 82.41  # E2
    voices = [root, root * 1.5, root * 2.0, root * 0.5, root * 3.0]  # E2, B2, E3, E1, B3
    detunes = [1.0, 1.003, 0.997, 1.0, 1.005]
    raw = np.zeros(n)
    for f, dt in zip(voices, detunes):
        # saw via harmonic sum, richer low end
        for k in range(1, 16):
            raw += (np.sin(2 * np.pi * f * dt * k * tt) / k) * (0.6 if f < 80 else 1.0)
    raw /= np.max(np.abs(raw))
    # dedicated sub sines for "final chord" low-end welly
    sub = 0.7 * np.sin(2 * np.pi * root * tt) + 0.5 * np.sin(2 * np.pi * root * 0.5 * tt)
    raw = raw + 0.55 * sub
    raw /= np.max(np.abs(raw))
    # amp env: near-instant attack, ~1.2s decay
    e = env_ad(n, 0.003, 1.2, curve=2.6)
    body = raw * e
    # power-amp grit: push into tanh, then a touch of hard clip
    gritty = sat(body * 3.2, 1.0)
    gritty = np.clip(gritty * 1.15, -1.0, 1.0)
    # tame the very top so it's a stab, not pure fizz
    gritty = one_pole_lp(gritty, 5200.0)
    out = 0.82 * gritty  # leave deliberate grit
    peak = np.max(np.abs(out))
    return out * (10 ** (-2.5 / 20) / peak)  # headroom for Vorbis overshoot


def make_beat_click():
    dur = 0.12
    n = int(dur * SR)
    tt = t(n)
    tone = np.sin(2 * np.pi * 1750 * tt) + 0.4 * np.sin(2 * np.pi * 2640 * tt)
    e = env_ad(n, 0.0006, 0.02, curve=7.0)
    sig = tone * e
    return normalize(sig, -12.0)  # quiet / unobtrusive


def make_build():
    rng = np.random.default_rng(2002)
    dur = 0.6
    n = int(dur * SR)
    buf = np.zeros(n)

    def clunk(at, tau=0.05, f=190.0):
        m = int(0.12 * SR)
        tt = t(m)
        body = np.sin(2 * np.pi * f * tt) * np.exp(-tt / tau)
        noise = one_pole_lp(rng.standard_normal(m), 900.0) * np.exp(-tt / 0.02)
        seg = 0.7 * body + 0.6 * noise
        s = int(at * SR)
        buf[s:s + m] += seg[: max(0, n - s)]

    clunk(0.0, tau=0.05, f=210.0)
    clunk(0.13, tau=0.06, f=150.0)
    # rising synth blip resolving up
    m = int(0.3 * SR)
    tt = t(m)
    f = np.linspace(420.0, 900.0, m)
    f[-int(0.06 * SR):] = 900.0  # settle on the resolved note
    phase = 2 * np.pi * np.cumsum(f) / SR
    blip = (np.sin(phase) + 0.3 * np.sin(2 * phase)) * env_ad(m, 0.005, 0.22, curve=2.5)
    s = int(0.28 * SR)
    buf[s:s + m] += 0.5 * blip[: max(0, n - s)]
    return normalize(buf, -3.0)


def make_scrap():
    dur = 0.15
    n = int(dur * SR)
    buf = np.zeros(n)
    notes = [(1318.5, 0.0, 0.05), (1975.5, 0.045, 0.09)]  # E6 -> B6
    for f, at, ln in notes:
        m = int(ln * SR)
        tt = t(m)
        tone = np.sin(2 * np.pi * f * tt) + 0.25 * np.sin(2 * np.pi * 2 * f * tt)
        seg = tone * env_ad(m, 0.002, ln * 0.6, curve=4.0)
        s = int(at * SR)
        buf[s:s + m] += seg[: max(0, n - s)]
    return normalize(buf, -3.0)


def make_pop(rng):
    """A bad vibe bursting: a short pitch-dropping blip with a wet click on the
    front. Deliberately *dry and small* — this fires several times a second in
    a busy wave, so anything with a tail or a tonal centre would turn the fight
    into mush and would fight the stems for the same frequencies. All the
    character is in the 40 ms pitch drop.
    """
    dur = 0.12
    n = int(dur * SR)
    tt = t(n)
    # Pitch drop: 900 Hz down to ~120 Hz in the first 40 ms.
    f = 120 + 780 * np.exp(-28 * tt)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * env_ad(n, 0.0008, 0.045, curve=4.5)
    # A short noise transient for the "pop" of the burst itself.
    click = rng.normal(0, 1, n)
    click = one_pole_lp(click, 3200) * env_ad(n, 0.0003, 0.012, curve=7.0)
    return normalize(sat(body * 0.9 + click * 0.55, 1.6), -6.0)


def make_music_loop():
    """2 bars at 124 BPM, seamless. Overflow tails are folded back to the
    start so the buffer is exactly periodic -> no click at the loop point."""
    rng = np.random.default_rng(4004)
    beats = 8
    L = int(round(beats * BEAT * SR))
    tail = int(0.5 * SR)
    xf = int(round(0.012 * SR))  # 12ms equal-length crossfade for a seamless seam
    buf = np.zeros(L + tail)

    def place(seg, at_beat):
        s = int(round(at_beat * BEAT * SR))
        e = s + len(seg)
        if e <= len(buf):
            buf[s:e] += seg
        else:
            buf[s:] += seg[: len(buf) - s]

    # --- drums --- (beat `beats` == start of next loop: continuation for the seam)
    for b in (0, 2, 4, 6, 8):
        place(0.9 * kick(), b)
    place(0.5 * kick(), 3.5)  # pickup
    for b in (1, 3, 5, 7):
        place(0.7 * snare(rng), b)
    for i in range(beats * 2 + 1):  # 8th-note hats (+1 lands on the seam)
        b = i * 0.5
        place(0.2 * hat(rng, open_=(i % 4 == 3)), b)

    # --- bass: E minor groove, roots E/G/A/D ---
    E1, G1, A1, D2 = 41.20, 49.00, 55.00, 73.42
    pattern = [
        (E1 * 2, 0.0, 0.75), (E1 * 2, 0.75, 0.25), (G1 * 2, 1.5, 0.5),
        (E1 * 2, 2.0, 0.5), (A1 * 2, 3.0, 0.75), (E1 * 2, 3.75, 0.25),
        (E1 * 2, 4.0, 0.75), (E1 * 2, 4.75, 0.25), (G1 * 2, 5.5, 0.5),
        (D2, 6.0, 0.75), (D2, 6.75, 0.25), (A1 * 2, 7.0, 0.75),
    ]
    for freq, at, ln in pattern:
        place(0.9 * bass_note(freq, ln * BEAT * 1.05), at)
    place(0.9 * bass_note(E1 * 2, 0.75 * BEAT * 1.05), 8.0)  # seam continuation

    # gentle bus glue + keep it warm at low volume (roll off harsh top).
    # Done on the whole rendered buffer BEFORE the seam crossfade so the
    # crossfade stays the final, seam-guaranteeing operation.
    buf = sat(buf * 1.1, 1.0)
    buf = one_pole_lp(buf, 12000.0)
    buf *= 10 ** (-3.5 / 20) / np.max(np.abs(buf[:L]))

    # equal-length crossfade of the loop head with the continuation past L,
    # so loop[-1] -> loop[0] is sample-continuous (buf[L] follows buf[L-1]).
    fade = np.linspace(0.0, 1.0, xf)
    loop = buf[:L].copy()
    loop[:xf] = buf[:xf] * fade + buf[L:L + xf] * (1.0 - fade)
    return loop


# ----------------------------------------------------------------------------
def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    print(f"writing to {OUT_DIR}")
    stems = {
        "count_in.ogg": make_count_in(),
        "chord_stab.ogg": make_chord_stab(),
        "beat_click.ogg": make_beat_click(),
        "build.ogg": make_build(),
        "scrap.ogg": make_scrap(),
        "pop.ogg": make_pop(np.random.default_rng(90210)),
        "music_loop.ogg": make_music_loop(),
    }
    paths = {name: write_ogg(name, data) for name, data in stems.items()}

    # loop-seam check
    ml = stems["music_loop.ogg"]
    print("\nmusic_loop seam check (should be near-continuous):")
    print("  first 4 samples:", np.round(ml[:4], 5).tolist())
    print("  last 4 samples :", np.round(ml[-4:], 5).tolist())
    print(f"  |last - first| = {abs(float(ml[-1]) - float(ml[0])):.6f}")

    print("\nsoundfile.info:")
    for name, p in paths.items():
        i = sf.info(p)
        print(f"  {name:16s} {i.format}/{i.subtype}  {i.samplerate} Hz  {i.channels}ch  {i.duration:.3f}s")


if __name__ == "__main__":
    main()
