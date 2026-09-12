#!/usr/bin/env python3
"""
gen_stems.py - synthesize the layered stem set for `shooty`'s returning music.

Run from the repo root:
    .venv/bin/python tools/gen_stems.py

The premise (see `src/game/gloom.rs`): the music has gone, and five joyless
townspeople are leaking bad vibes into the streets. Cheer one up and *their*
part of the track comes back. So there are five stems, one per crowd genre in
`skins::BOXY`, and the player assembles the song by clearing the map.

That makes two demands the usual "one music loop" does not:

  1. **Every stem has to hold up alone.** You might cure the country busker
     first and hear nothing but a twangy lick for a minute. So no stem is a
     pad-under-the-real-part; each one carries its own rhythm and its own hook.
  2. **Every *subset* has to work.** The player picks the order, so all 31
     non-empty combinations get heard. They are written to one grid (124 BPM,
     A minor, 8 bars, i-VI-III-VII) and deliberately separated by register and
     by where they land in the bar, so nothing fights:

        hiphop   broadband, on the beat      boom-bap kit
        techno   sub + low mid, 16ths        rolling acid bass
        disco    mid, off-beat               clav + string stabs, shaker
        country  mid-high, syncopated        Karplus-Strong guitar lick
        jpop     high, 16ths + sustained     arp and lead melody

Pure-numpy synthesis, deterministic (all noise seeded), idempotent. Writes
five OGG/Vorbis files of **identical sample length** into assets/audio/stems/ -
that equal length is what lets the game start all five at once, hold them at
zero volume and fade any of them up at any time while they stay phase-locked.

Encode path is the same as tools/gen_audio.py: numpy float32 ->
soundfile.write(..., format='OGG', subtype='VORBIS'), because ffmpeg is not
installed on this machine.
"""

from __future__ import annotations

import os

import numpy as np
import soundfile as sf

SR = 44100
BPM = 124.0
BEAT = 60.0 / BPM  # 0.483871s
BAR = 4 * BEAT
BARS = 8
TOTAL = int(round(BARS * BAR * SR))  # every stem is exactly this long

OUT_DIR = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "assets", "audio", "stems"
)

# i - VI - III - VII in A minor, two bars each. Semitones from A.
# Am (A C E), F (F A C), C (C E G), G (G B D)
PROG = [
    ("Am", 0, [0, 3, 7]),
    ("F", -4, [0, 4, 7]),
    ("C", 3, [0, 4, 7]),
    ("G", -2, [0, 4, 7]),
]
A2 = 110.0


def hz(semis, octave=0):
    """Frequency of `semis` semitones above A2, transposed `octave` octaves."""
    return A2 * (2.0 ** (semis / 12.0)) * (2.0**octave)


def chord_at(beat):
    """Which progression entry is sounding at `beat` (0..32)."""
    return PROG[int(beat // 8) % 4]


# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------
def buf():
    return np.zeros(TOTAL, dtype=np.float64)


def at(beat):
    """Sample index of a beat position."""
    return int(round(beat * BEAT * SR))


def add(dst, src, beat, gain=1.0):
    """Mix `src` into `dst` at `beat`, wrapping past the end so the loop is
    seamless - a tail that runs off the last bar lands back on the first."""
    i = at(beat)
    n = len(src)
    if i >= TOTAL:
        i %= TOTAL
    end = i + n
    if end <= TOTAL:
        dst[i:end] += src * gain
    else:
        head = TOTAL - i
        dst[i:] += src[:head] * gain
        wrap = min(n - head, TOTAL)
        dst[:wrap] += src[head : head + wrap] * gain


def t(n):
    return np.arange(n) / SR


def env_ad(n, attack, decay, curve=3.0):
    e = np.ones(n)
    a = max(1, int(attack * SR))
    if a > 1:
        e[:a] = np.linspace(0.0, 1.0, a)
    d = np.arange(n)
    e *= np.exp(-curve * np.clip((d - a) / max(1, decay * SR), 0, None))
    return e


def env_adsr(n, a, d, s, r):
    """Attack-decay-sustain-release over exactly n samples."""
    a_n, d_n, r_n = (max(1, int(x * SR)) for x in (a, d, r))
    a_n = min(a_n, n)
    d_n = min(d_n, max(0, n - a_n))
    r_n = min(r_n, max(0, n - a_n - d_n))
    sus_n = max(0, n - a_n - d_n - r_n)
    return np.concatenate(
        [
            np.linspace(0, 1, a_n),
            np.linspace(1, s, d_n),
            np.full(sus_n, s),
            np.linspace(s, 0, r_n),
        ]
    )[:n]


def one_pole_lp(x, cutoff):
    dt = 1.0 / SR
    rc = 1.0 / (2 * np.pi * max(cutoff, 20.0))
    a = dt / (rc + dt)
    y = np.empty_like(x)
    acc = 0.0
    for i in range(len(x)):
        acc += a * (x[i] - acc)
        y[i] = acc
    return y


def one_pole_hp(x, cutoff):
    return x - one_pole_lp(x, cutoff)


def svf_lp(x, cutoff, q=1.0):
    """State-variable lowpass with a per-sample cutoff array - the resonant
    sweep the acid bass is built on. `cutoff` may be scalar or per-sample."""
    n = len(x)
    fc = np.broadcast_to(np.asarray(cutoff, dtype=np.float64), (n,))
    f = 2.0 * np.sin(np.pi * np.clip(fc, 20.0, SR * 0.45) / SR)
    damp = 1.0 / max(q, 0.5)
    low = band = 0.0
    out = np.empty(n)
    for i in range(n):
        high = x[i] - low - damp * band
        band += f[i] * high
        low += f[i] * band
        out[i] = low
    return out


def saw(freq, n, phase=0.0):
    ph = phase + np.cumsum(np.full(n, freq / SR))
    return 2.0 * (ph % 1.0) - 1.0


def square(freq, n, duty=0.5, phase=0.0):
    ph = phase + np.cumsum(np.full(n, freq / SR))
    return np.where((ph % 1.0) < duty, 1.0, -1.0)


def tri(freq, n, phase=0.0):
    ph = phase + np.cumsum(np.full(n, freq / SR))
    return 4.0 * np.abs((ph % 1.0) - 0.5) - 1.0


def sine(freq, n, phase=0.0):
    ph = phase + np.cumsum(np.full(n, freq / SR))
    return np.sin(2 * np.pi * ph)


def karplus(freq, n, rng, damp=0.5, bright=0.5):
    """Plucked string. A noise burst round a delay line, low-passed each lap -
    this is what makes the country stem sound picked rather than beeped."""
    ln = max(2, int(SR / freq))
    buf_ = rng.uniform(-1, 1, ln)
    buf_ = buf_ * np.linspace(1.0, bright, ln)
    out = np.empty(n)
    idx = 0
    prev = 0.0
    for i in range(n):
        cur = buf_[idx]
        nxt = buf_[(idx + 1) % ln]
        filt = (cur + nxt) * 0.5
        filt = prev + (filt - prev) * (0.35 + 0.6 * bright)
        prev = filt
        buf_[idx] = filt * (0.999 - 0.02 * damp)
        out[i] = cur
        idx = (idx + 1) % ln
    return out


def soft_clip(x, drive=1.0):
    return np.tanh(x * drive)


def normalize(x, peak=0.5):
    m = np.max(np.abs(x))
    return x if m < 1e-9 else x * (peak / m)


def swing(beat, amount=0.055):
    """Nudge off-beat 8ths later - the boom-bap shuffle."""
    return beat + (amount if abs((beat * 2) % 2 - 1) < 1e-6 else 0.0)


# ---------------------------------------------------------------------------
# hiphop - boom-bap kit. The floor of the whole track, and a break in its own
# right: swung hats, a fat snare on 2 and 4, and a kick that syncopates rather
# than marching.
# ---------------------------------------------------------------------------
def stem_hiphop(rng):
    out = buf()

    def kick():
        n = int(0.34 * SR)
        f = 52 * np.exp(-13 * t(n)) + 44
        body = np.sin(2 * np.pi * np.cumsum(f) / SR) * env_ad(n, 0.001, 0.13, 3.2)
        click = rng.normal(0, 1, n) * env_ad(n, 0.0002, 0.006, 6.0) * 0.35
        return soft_clip(body * 1.2 + click, 1.4) * 0.95

    def snare(soft=False):
        n = int(0.24 * SR)
        noise = rng.normal(0, 1, n)
        noise = one_pole_hp(noise, 1100) * env_ad(n, 0.0008, 0.085 if not soft else 0.05, 3.0)
        tone = (sine(188, n) + sine(268, n) * 0.7) * env_ad(n, 0.0005, 0.05, 4.0)
        return (noise * 0.9 + tone * 0.45) * (0.85 if not soft else 0.4)

    def hat(open_=False):
        n = int((0.16 if open_ else 0.05) * SR)
        h = one_pole_hp(rng.normal(0, 1, n), 7200)
        return h * env_ad(n, 0.0003, 0.1 if open_ else 0.02, 3.5) * (0.3 if open_ else 0.42)

    for bar in range(BARS):
        b0 = bar * 4
        # Kick: 1, the "and" of 2, and a pickup before 4 - classic boom bap.
        add(out, kick(), b0 + 0.0)
        add(out, kick(), b0 + 1.5, 0.85)
        if bar % 2 == 1:
            add(out, kick(), b0 + 3.25, 0.7)
        else:
            add(out, kick(), b0 + 2.75, 0.6)
        # Snare backbeat, with a ghost note leading into it.
        add(out, snare(), b0 + 1.0)
        add(out, snare(), b0 + 3.0)
        add(out, snare(soft=True), b0 + 2.5, 0.5)
        if bar % 4 == 3:
            add(out, snare(soft=True), b0 + 3.5, 0.6)
            add(out, snare(soft=True), b0 + 3.75, 0.8)
        # Swung 8th hats, open on the last 8th of the bar.
        for k in range(8):
            b = swing(b0 + k * 0.5)
            add(out, hat(open_=(k == 7 and bar % 2 == 1)), b, 0.9 if k % 2 == 0 else 0.6)

    # A whisper of vinyl so it sits like a sampled break rather than a drum machine.
    hisssrc = one_pole_hp(rng.normal(0, 1, TOTAL), 4000) * 0.006
    return normalize(soft_clip(out + hisssrc, 1.15), 0.62)


# ---------------------------------------------------------------------------
# techno - a rolling 16th acid bass with a resonant filter sweep, plus the
# off-beat hat that is the genre's whole feel. Carries a pulse on its own.
# ---------------------------------------------------------------------------
def stem_techno(rng):
    out = buf()
    step = 0.25  # 16ths
    steps = int(BARS * 4 / step)

    # A 16-step pattern: which steps sound, and which are an octave up.
    gate = [1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1, 0, 1, 1, 0, 1]
    up = [0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0]

    for s in range(steps):
        beat = s * step
        if not gate[s % 16]:
            continue
        _, root, _ = chord_at(beat)
        oct_ = 1 if up[s % 16] else 0
        f = hz(root, oct_)
        n = int(0.22 * SR)
        # Sweep the cutoff across the loop, twice, so it breathes.
        phase = (beat / (BARS * 4)) * 2.0
        base = 340 + 1500 * (0.5 - 0.5 * np.cos(2 * np.pi * phase))
        env = env_ad(n, 0.002, 0.09, 3.0)
        cutoff = base * (0.7 + 2.4 * env)
        raw = saw(f, n) * 0.7 + square(f, n, 0.42) * 0.3
        v = svf_lp(raw * env, cutoff, q=3.2)
        add(out, soft_clip(v, 2.2), beat, 0.55)

    # Sub, so the bass has weight under the filter.
    for bar in range(BARS):
        b0 = bar * 4
        for k in (0, 2):
            _, root, _ = chord_at(b0 + k)
            n = int(0.5 * SR)
            add(out, sine(hz(root, -1), n) * env_ad(n, 0.004, 0.22, 2.6), b0 + k, 0.5)

    # Off-beat open hat - quiet, so it drives without crowding the boom-bap kit.
    for bar in range(BARS):
        for k in range(4):
            n = int(0.13 * SR)
            h = one_pole_hp(rng.normal(0, 1, n), 6800) * env_ad(n, 0.0004, 0.075, 3.0)
            add(out, h, bar * 4 + k + 0.5, 0.16)

    return normalize(out, 0.55)


# ---------------------------------------------------------------------------
# disco - off-beat clav stabs, a string swell answering them, and a 16th
# shaker. Sits in the mid, and always *between* the beats, so it interlocks
# with the kit instead of doubling it.
# ---------------------------------------------------------------------------
def stem_disco(rng):
    out = buf()

    def clav(freqs, dur):
        n = int(dur * SR)
        v = np.zeros(n)
        for f in freqs:
            v += square(f, n, 0.28) * 0.4 + saw(f, n) * 0.2
        v = svf_lp(v, 2600, q=1.4) * env_ad(n, 0.0015, 0.055, 4.5)
        return v / max(1, len(freqs) ** 0.5)

    def strings(freqs, dur):
        n = int(dur * SR)
        v = np.zeros(n)
        for f in freqs:
            # Three detuned saws = the string-machine shimmer.
            for d in (-0.16, 0.0, 0.19):
                v += saw(f * (1 + d / 100.0), n, phase=rng.random())
        v = svf_lp(v, 3000, q=0.9)
        return v * env_adsr(n, 0.09, 0.12, 0.75, 0.2) / (len(freqs) * 3)

    for bar in range(BARS):
        b0 = bar * 4
        _, root, ivs = chord_at(b0)
        # Clav on every off-beat 8th - the disco "chika".
        notes = [hz(root + i, 1) for i in ivs]
        for k in range(4):
            add(out, clav(notes, 0.22), b0 + k + 0.5, 0.5 if k % 2 else 0.62)
        # A short skank on the "e" of 3 for lift.
        add(out, clav([hz(root + ivs[-1], 1)], 0.16), b0 + 2.25, 0.35)
        # Strings: a long chord over the first half, a stabbed answer after.
        add(out, strings(notes, BEAT * 1.75), b0 + 0.0, 0.42)
        add(out, strings([f * 1.5 for f in notes], BEAT * 0.9), b0 + 2.5, 0.3)

    # 16th shaker with an accent pattern.
    for s in range(int(BARS * 16)):
        n = int(0.045 * SR)
        sh = one_pole_hp(rng.normal(0, 1, n), 8500) * env_ad(n, 0.0006, 0.018, 4.0)
        accent = 1.0 if s % 4 == 2 else 0.45
        add(out, sh, s * 0.25, 0.12 * accent)

    return normalize(out, 0.5)


# ---------------------------------------------------------------------------
# country - a plucked pentatonic lick on a Karplus-Strong string, with a
# hammer-on pair and a bent double-stop. Syncopated against everything else,
# and the one stem with an actual tune you could hum.
# ---------------------------------------------------------------------------
def stem_country(rng):
    out = buf()

    def pluck(semis, dur, oct_=1, gain=1.0, bright=0.62):
        n = int(dur * SR)
        v = karplus(hz(semis, oct_), n, rng, damp=0.45, bright=bright)
        v *= env_adsr(n, 0.002, 0.05, 0.72, min(0.25, dur * 0.5))
        return v * gain

    # A minor pentatonic phrase per chord, restated with variation.
    # (beat within the 8-bar loop, semitone above A, length in beats)
    lick = [
        (0.0, 0, 0.5), (0.5, 3, 0.5), (1.0, 5, 0.75), (1.75, 7, 0.75),
        (3.0, 5, 0.5), (3.5, 3, 0.5),
        (4.0, 0, 0.75), (5.0, -2, 0.5), (5.5, 0, 1.0), (7.0, 3, 1.0),
        # bar 3-4 over F: lean on the major third
        (8.0, 8, 0.5), (8.5, 5, 0.5), (9.0, 8, 0.75), (9.75, 12, 0.75),
        (11.0, 8, 1.0),
        (12.0, 5, 0.75), (13.0, 3, 0.5), (13.5, 5, 1.5),
        # bar 5-6 over C
        (16.0, 3, 0.5), (16.5, 7, 0.5), (17.0, 10, 0.75), (17.75, 7, 0.75),
        (19.0, 3, 1.0),
        (20.0, 7, 0.5), (20.5, 10, 0.5), (21.0, 12, 1.5), (22.5, 10, 0.75),
        # bar 7-8 over G: walk back down to the top of the loop
        (24.0, 10, 0.5), (24.5, 7, 0.5), (25.0, 5, 0.75), (25.75, 3, 0.75),
        (27.0, 2, 1.0),
        (28.0, 3, 0.5), (28.5, 5, 0.5), (29.0, 7, 1.0), (30.0, 5, 0.5),
        (30.5, 3, 0.5), (31.0, 0, 1.0),
    ]
    for beat, semis, dur in lick:
        add(out, pluck(semis, dur * BEAT * 1.15), beat, 0.55)
        # Double-stop a third above on the long notes - the country twang.
        if dur >= 1.0:
            add(out, pluck(semis + 3, dur * BEAT * 0.9, gain=0.4), beat + 0.03, 0.5)

    # A low root on the downbeat of each chord change, muted and short.
    for bar in range(0, BARS, 2):
        _, root, _ = chord_at(bar * 4)
        add(out, pluck(root, 0.55, oct_=0, gain=0.7, bright=0.3), bar * 4, 0.5)

    out = svf_lp(out, 5200, q=0.8)
    return normalize(out, 0.5)


# ---------------------------------------------------------------------------
# jpop - a bright 16th arpeggio plus a sustained hook an octave up. Lives at
# the top of the spectrum, which is exactly where nothing else is.
# ---------------------------------------------------------------------------
def stem_jpop(rng):
    out = buf()

    def bell(f, dur, gain=1.0):
        n = int(dur * SR)
        # Detuned triangle pair + a sine octave = a clean, glassy pluck.
        v = (
            tri(f, n) * 0.55
            + tri(f * 1.0035, n) * 0.45
            + sine(f * 2, n) * 0.22
        )
        return v * env_ad(n, 0.003, 0.11, 2.6) * gain

    def lead(f, dur, gain=1.0):
        n = int(dur * SR)
        vib = 1 + 0.004 * np.sin(2 * np.pi * 5.4 * t(n)) * np.clip(t(n) * 4, 0, 1)
        ph = np.cumsum(f * vib / SR)
        v = np.sin(2 * np.pi * ph) * 0.6 + 0.4 * (2 * ((ph * 1.0) % 1.0) - 1)
        v = svf_lp(v, 4200, q=0.9)
        return v * env_adsr(n, 0.02, 0.1, 0.8, 0.14) * gain

    # Arp: up-down through the chord across 16ths, two octaves.
    shape = [0, 1, 2, 3, 2, 1, 2, 3]
    for s in range(int(BARS * 16)):
        beat = s * 0.25
        _, root, ivs = chord_at(beat)
        tones = [ivs[0], ivs[1], ivs[2], ivs[0] + 12]
        deg = tones[shape[s % len(shape)] % len(tones)]
        f = hz(root + deg, 2)
        add(out, bell(f, 0.3, gain=0.85 if s % 4 == 0 else 0.55), beat, 0.34)

    # Hook melody, sparse and singable, answering the arp.
    hook = [
        (0.0, 12, 1.5), (1.5, 15, 0.5), (2.0, 19, 2.0),
        (5.0, 17, 1.0), (6.0, 15, 2.0),
        (8.0, 12, 1.5), (9.5, 17, 0.5), (10.0, 20, 2.0),
        (13.0, 19, 1.0), (14.0, 17, 2.0),
        (16.0, 15, 1.0), (17.0, 19, 1.0), (18.0, 24, 2.0),
        (21.0, 22, 1.0), (22.0, 19, 2.0),
        (24.0, 14, 1.0), (25.0, 17, 1.0), (26.0, 22, 1.5),
        (27.5, 19, 0.5), (28.0, 17, 2.0), (30.0, 12, 2.0),
    ]
    for beat, semis, dur in hook:
        add(out, lead(hz(semis, 1), dur * BEAT * 0.95, gain=0.5), beat, 0.5)

    return normalize(out, 0.5)


# ---------------------------------------------------------------------------
STEMS = [
    ("hiphop", stem_hiphop),
    ("techno", stem_techno),
    ("disco", stem_disco),
    ("country", stem_country),
    ("jpop", stem_jpop),
]


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    rendered = {}
    for name, fn in STEMS:
        # Per-stem seed: re-rendering one stem must not shift the others.
        rng = np.random.default_rng(abs(hash(name)) % (2**31))
        rng = np.random.default_rng(sum(ord(c) * (i + 7) for i, c in enumerate(name)))
        x = fn(rng)
        assert len(x) == TOTAL, f"{name} is {len(x)} samples, expected {TOTAL}"
        # Tiny fade at the seam so the loop point cannot click.
        edge = int(0.004 * SR)
        x[:edge] *= np.linspace(0, 1, edge)
        x[-edge:] *= np.linspace(1, 0, edge)
        path = os.path.join(OUT_DIR, f"{name}.ogg")
        sf.write(path, x.astype(np.float32), SR, format="OGG", subtype="VORBIS")
        rendered[name] = x
        print(f"  {name:8s} -> {path}  ({len(x)} samples, {len(x)/SR:.3f}s)")

    mix = sum(rendered.values())
    print(f"\n  full mix peak {np.max(np.abs(mix)):.2f} (soft-clipped in game by volume)")
    print(f"  {BARS} bars @ {BPM:g} BPM = {TOTAL/SR:.3f}s, all stems identical length")


if __name__ == "__main__":
    main()
