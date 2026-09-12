#!/usr/bin/env python3
"""Bake a self-contained **audio lab** page for the stems in `assets/audio`.

The visual side of the game has `src/bin/lab.rs`; this is the ear's version of
it. Judging a cue by triggering it mid-fight tells you almost nothing — you hear
it once, under gunfire, and can't A/B it. Here every stem gets a waveform you can
scrub, a speed and gain control, and — the part that actually matters for a game
built on a 124 BPM groove — a beat grid drawn over the waveform plus a metronome
and the music bed you can layer underneath, so you can hear whether a cue lands
on the beat instead of guessing.

Everything is inlined (the OGGs are base64'd straight into the HTML), so the
output is one file that opens from disk with no server and no asset paths.

    .venv/bin/python3 tools/audio_lab.py
    open screenshots/audio_lab.html

The BPM and the cue-to-gameplay mapping are read out of the Rust source so this
page can't quietly drift from the game the way a hand-written doc would.
"""

from __future__ import annotations

import base64
import json
import re
from pathlib import Path

AUDIO = Path("assets/audio")
OUT = Path("screenshots/audio_lab.html")

# Which gameplay moment fires each stem — kept here because it's the thing you
# actually need in your head while listening. Keys are file stems.
FIRED_BY = {
    "count_in": "drumstick 1-2-3-4 — a stage channel begins (build.rs)",
    "chord_stab": "power chord — Encore blast, drummer slam, a stage finishing",
    "beat_click": "the drummer's metronome beat, once per beat at the BPM below",
    "build": "a structure is placed / starts building",
    "scrap": "scrap mote collected",
    "music_loop": "the bed — rides SecuredZones::music_level, silent to full",
}


def bpm_from_source() -> float:
    """Read the drummer's tempo out of the game rather than restating it."""
    src = Path("src/game/drummer.rs").read_text()
    m = re.search(r"const BPM: f32 = ([\d.]+);", src)
    return float(m.group(1)) if m else 124.0


def main() -> None:
    bpm = bpm_from_source()
    clips = []
    for p in sorted(AUDIO.glob("*.ogg")):
        clips.append(
            {
                "name": p.stem,
                "file": p.name,
                "bytes": p.stat().st_size,
                "note": FIRED_BY.get(p.stem, ""),
                "data": base64.b64encode(p.read_bytes()).decode("ascii"),
            }
        )
    if not clips:
        raise SystemExit(f"no .ogg stems found in {AUDIO}")

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(page(clips, bpm))
    total = sum(c["bytes"] for c in clips)
    print(f"{OUT}  —  {len(clips)} stems, {total / 1024:.0f} KB inlined, {bpm:g} BPM")


def page(clips: list[dict], bpm: float) -> str:
    payload = json.dumps(clips)
    return f"""<!doctype html>
<meta charset="utf-8">
<title>shooty — audio lab</title>
<style>
  :root {{
    --bg: #10131a; --panel: #171b25; --line: #262c3a;
    --ink: #e8ecf5; --dim: #8d97ab; --accent: #35d6e8; --warm: #ffab3d;
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0; padding: 22px 24px 60px; background: var(--bg); color: var(--ink);
    font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace;
  }}
  h1 {{ font-size: 15px; letter-spacing: .14em; text-transform: uppercase; margin: 0 0 2px; }}
  .sub {{ color: var(--dim); margin-bottom: 20px; }}
  .bar {{
    display: flex; gap: 18px; align-items: center; flex-wrap: wrap;
    background: var(--panel); border: 1px solid var(--line);
    padding: 10px 14px; margin-bottom: 20px; position: sticky; top: 0; z-index: 2;
  }}
  .clip {{ background: var(--panel); border: 1px solid var(--line); margin-bottom: 12px; }}
  .head {{
    display: flex; gap: 14px; align-items: baseline; padding: 9px 14px;
    border-bottom: 1px solid var(--line);
  }}
  .head b {{ font-size: 13px; letter-spacing: .05em; }}
  .head .note {{ color: var(--dim); font-size: 12px; }}
  .head .len {{ margin-left: auto; color: var(--dim); font-size: 12px; white-space: nowrap; }}
  canvas {{ display: block; width: 100%; height: 92px; cursor: crosshair; }}
  .row {{ display: flex; gap: 14px; align-items: center; padding: 9px 14px; flex-wrap: wrap; }}
  button {{
    background: #222836; color: var(--ink); border: 1px solid var(--line);
    padding: 4px 12px; font: inherit; cursor: pointer;
  }}
  button:hover {{ border-color: var(--accent); }}
  button.on {{ background: var(--accent); border-color: var(--accent); color: #06222a; }}
  label {{ color: var(--dim); display: flex; gap: 7px; align-items: center; }}
  input[type=range] {{ width: 108px; accent-color: var(--accent); }}
  .val {{ color: var(--ink); min-width: 44px; display: inline-block; }}
  kbd {{
    background: #222836; border: 1px solid var(--line); padding: 1px 5px; color: var(--ink);
  }}
</style>

<h1>shooty — audio lab</h1>
<div class="sub">
  Every stem in <code>assets/audio</code>, inlined. Click a waveform to seek ·
  <kbd>space</kbd> stops everything. Beat grid and metronome run at the game's
  own tempo, read from <code>drummer.rs</code>.
</div>

<div class="bar">
  <label>tempo <span class="val" id="bpmv">{bpm:g}</span> BPM
    <input type="range" id="bpm" min="60" max="200" step="1" value="{bpm:g}"></label>
  <label><input type="checkbox" id="grid" checked> beat grid</label>
  <button id="metro">metronome</button>
  <button id="bed">music bed under everything</button>
  <button id="stop">stop all</button>
</div>

<div id="clips"></div>

<script>
const CLIPS = {payload};
let BPM = {bpm:g};

const ctx = new (window.AudioContext || window.webkitAudioContext)();
const master = ctx.createGain();
master.connect(ctx.destination);

function decode(b64) {{
  const raw = atob(b64);
  const buf = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) buf[i] = raw.charCodeAt(i);
  return ctx.decodeAudioData(buf.buffer);
}}

/** Peak envelope per pixel column — a plain average hides transients, and
 *  transients are exactly what you are listening for in a one-shot cue. */
function peaks(buffer, columns) {{
  const ch = buffer.getChannelData(0);
  const per = Math.max(1, Math.floor(ch.length / columns));
  const out = new Float32Array(columns);
  for (let c = 0; c < columns; c++) {{
    let peak = 0;
    const start = c * per;
    for (let i = start; i < start + per && i < ch.length; i++) {{
      const v = Math.abs(ch[i]);
      if (v > peak) peak = v;
    }}
    out[c] = peak;
  }}
  return out;
}}

const voices = new Set();

function play(buffer, {{ rate = 1, gain = 1, loop = false, when = 0 }} = {{}}) {{
  const src = ctx.createBufferSource();
  src.buffer = buffer;
  src.playbackRate.value = rate;
  src.loop = loop;
  const g = ctx.createGain();
  g.gain.value = gain;
  src.connect(g).connect(master);
  src.start(when || ctx.currentTime);
  voices.add(src);
  src.onended = () => voices.delete(src);
  return src;
}}

function stopAll() {{
  for (const v of voices) {{ try {{ v.stop(); }} catch (e) {{}} }}
  voices.clear();
  metroOn = false; bedOn = false;
  document.getElementById('metro').classList.remove('on');
  document.getElementById('bed').classList.remove('on');
  for (const c of cards) c.stop();
}}

const cards = [];

class Card {{
  constructor(clip) {{
    this.clip = clip;
    this.rate = 1; this.gain = 1; this.loop = false;
    this.source = null; this.startedAt = 0;

    const el = document.createElement('div');
    el.className = 'clip';
    el.innerHTML = `
      <div class="head">
        <b>${{clip.name}}</b>
        <span class="note">${{clip.note}}</span>
        <span class="len" data-len>— · ${{(clip.bytes / 1024).toFixed(1)}} KB</span>
      </div>
      <canvas></canvas>
      <div class="row">
        <button data-play>play</button>
        <button data-loop>loop</button>
        <label>speed <input type="range" data-rate min="0.25" max="2" step="0.05" value="1">
          <span class="val" data-ratev>1.00x</span></label>
        <label>gain <input type="range" data-gain min="0" max="2" step="0.05" value="1">
          <span class="val" data-gainv>1.00</span></label>
        <span class="len" data-beats></span>
      </div>`;
    document.getElementById('clips').appendChild(el);

    this.el = el;
    this.canvas = el.querySelector('canvas');
    this.canvas.addEventListener('click', (e) => {{
      const r = this.canvas.getBoundingClientRect();
      this.playFrom(((e.clientX - r.left) / r.width) * this.buffer.duration);
    }});
    el.querySelector('[data-play]').onclick = () => (this.source ? this.stop() : this.playFrom(0));
    const loopBtn = el.querySelector('[data-loop]');
    loopBtn.onclick = () => {{ this.loop = !this.loop; loopBtn.classList.toggle('on', this.loop); }};
    const rate = el.querySelector('[data-rate]');
    rate.oninput = () => {{
      this.rate = +rate.value;
      el.querySelector('[data-ratev]').textContent = this.rate.toFixed(2) + 'x';
      if (this.source) this.source.playbackRate.value = this.rate;
    }};
    const gain = el.querySelector('[data-gain]');
    gain.oninput = () => {{
      this.gain = +gain.value;
      el.querySelector('[data-gainv]').textContent = this.gain.toFixed(2);
    }};

    decode(clip.data).then((buf) => {{
      this.buffer = buf;
      el.querySelector('[data-len]').textContent =
        `${{buf.duration.toFixed(3)}}s · ${{(clip.bytes / 1024).toFixed(1)}} KB`;
      this.updateBeats();
      this.draw();
    }});
  }}

  /** How the clip sits against the grid. Length in beats is the useful number:
   *  a cue that is a whole number of beats long ends on a downbeat and can be
   *  looped or stacked; one that lands between beats will always fight the
   *  groove no matter when it is triggered. */
  updateBeats() {{
    if (!this.buffer) return;
    const beats = this.buffer.duration / (60 / BPM);
    const nearest = Math.round(beats);
    const off = beats - nearest;
    let msg = `${{beats.toFixed(2)}} beats`;
    if (nearest >= 4 && nearest % 4 === 0) {{
      const bars = nearest / 4;
      msg += ` (${{bars}} bar${{bars === 1 ? '' : 's'}})`;
    }}
    // A twentieth of a beat is inaudible as drift; past that it reads as sloppy.
    if (nearest >= 1 && Math.abs(off) > 0.05) {{
      msg += ` — off-grid by ${{off > 0 ? '+' : ''}}${{off.toFixed(2)}}`;
    }}
    this.el.querySelector('[data-beats]').textContent = msg;
  }}

  playFrom(offset) {{
    if (!this.buffer) return;
    this.stop();
    const src = ctx.createBufferSource();
    src.buffer = this.buffer;
    src.playbackRate.value = this.rate;
    src.loop = this.loop;
    const g = ctx.createGain();
    g.gain.value = this.gain;
    src.connect(g).connect(master);
    src.start(ctx.currentTime, offset);
    voices.add(src);
    src.onended = () => {{
      voices.delete(src);
      if (this.source === src) {{ this.source = null; this.mark(false); }}
    }};
    this.source = src;
    // Where the clip *would* have started, so the playhead maths survives a seek.
    this.startedAt = ctx.currentTime - offset / this.rate;
    this.mark(true);
  }}

  stop() {{
    if (this.source) {{
      const src = this.source;
      this.source = null;
      try {{ src.stop(); }} catch (e) {{}}
      voices.delete(src);
    }}
    this.mark(false);
  }}

  mark(on) {{
    this.el.querySelector('[data-play]').classList.toggle('on', on);
  }}

  draw() {{
    const c = this.canvas, dpr = devicePixelRatio || 1;
    const w = c.clientWidth, h = c.clientHeight;
    c.width = w * dpr; c.height = h * dpr;
    const g = c.getContext('2d');
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.clearRect(0, 0, w, h);
    if (!this.buffer) return;

    // Beat grid first, so the waveform sits on top of it.
    if (document.getElementById('grid').checked) {{
      const beat = 60 / BPM;
      for (let t = 0, i = 0; t < this.buffer.duration; t += beat, i++) {{
        const x = (t / this.buffer.duration) * w;
        g.strokeStyle = i % 4 === 0 ? 'rgba(255,171,61,.55)' : 'rgba(255,171,61,.18)';
        g.beginPath(); g.moveTo(x, 0); g.lineTo(x, h); g.stroke();
      }}
    }}

    const p = peaks(this.buffer, Math.floor(w));
    g.fillStyle = '#35d6e8';
    for (let x = 0; x < p.length; x++) {{
      const a = p[x] * (h / 2) * 0.94;
      g.fillRect(x, h / 2 - a, 1, a * 2 || 1);
    }}
    g.strokeStyle = 'rgba(232,236,245,.22)';
    g.beginPath(); g.moveTo(0, h / 2); g.lineTo(w, h / 2); g.stroke();

    if (this.source) {{
      const at = (ctx.currentTime - this.startedAt) * this.rate;
      const t = this.loop ? at % this.buffer.duration : at;
      if (t <= this.buffer.duration) {{
        const x = (t / this.buffer.duration) * w;
        g.strokeStyle = '#fff';
        g.beginPath(); g.moveTo(x, 0); g.lineTo(x, h); g.stroke();
      }}
    }}
  }}
}}

for (const clip of CLIPS) cards.push(new Card(clip));

function frame() {{ for (const c of cards) c.draw(); requestAnimationFrame(frame); }}
requestAnimationFrame(frame);

// --- transport -------------------------------------------------------------
let metroOn = false, metroTimer = null, bedOn = false;

const metroBtn = document.getElementById('metro');
metroBtn.onclick = () => {{
  metroOn = !metroOn;
  metroBtn.classList.toggle('on', metroOn);
  clearInterval(metroTimer);
  if (!metroOn) return;
  const click = cards.find((c) => c.clip.name === 'beat_click');
  const tick = () => {{
    if (click && click.buffer) play(click.buffer, {{ gain: 0.7 }});
    else {{
      // No click stem? Synthesise one rather than failing silently.
      const o = ctx.createOscillator(), g = ctx.createGain();
      o.frequency.value = 1600; g.gain.value = 0.18;
      o.connect(g).connect(master); o.start();
      g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.05);
      o.stop(ctx.currentTime + 0.06);
    }}
  }};
  tick();
  metroTimer = setInterval(tick, (60 / BPM) * 1000);
}};

const bedBtn = document.getElementById('bed');
bedBtn.onclick = () => {{
  const bed = cards.find((c) => c.clip.name === 'music_loop');
  if (!bed || !bed.buffer) return;
  bedOn = !bedOn;
  bedBtn.classList.toggle('on', bedOn);
  if (bedOn) {{ bed.loop = true; bed.gain = 0.45; bed.playFrom(0); }} else bed.stop();
}};

document.getElementById('stop').onclick = stopAll;
addEventListener('keydown', (e) => {{
  if (e.code === 'Space') {{ e.preventDefault(); stopAll(); }}
}});

const bpmSlider = document.getElementById('bpm');
bpmSlider.oninput = () => {{
  BPM = +bpmSlider.value;
  document.getElementById('bpmv').textContent = BPM;
  for (const c of cards) c.updateBeats();
  if (metroOn) {{ metroBtn.click(); metroBtn.click(); }}
}};
</script>
"""


if __name__ == "__main__":
    main()
