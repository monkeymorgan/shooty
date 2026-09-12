# Shooty

A 3D low-poly arena survival shooter — prototype from `DESIGN.md`.
Theme: rockers bringing rock music back to a town overrun by every other genre.

## Art direction — boxy, near-voxel, all of it

**One rule, and it governs characters, environment, props and VFX alike: it is
built out of boxes.** Chunky rectangular volumes, flat colour, painted detail
rather than modelled detail, minimal faces, soft shadows, a bright toy palette.
If a thing can be read as a stack of boxes, it should be built as one.

Reference points, in the order they matter:

| Reference | What we take from it |
| --- | --- |
| **Synty "Simple People"** (`assets/refs/characters.jpg`) | The target for *characters*: box head, slab hair, block limbs, two dark bars and a line for a face. The look we are aiming at — but the packs are paid, so we reproduce the style rather than buy it. |
| **BAM Squad** (VR) | Proportions and readability in motion: big head, chunky torso, stubby limbs, one loud colour per character. |
| **Job Simulator** (VR) | The "everything is a friendly box" world rule — props, machines and set dressing built from the same vocabulary as the people, never a fussier one. |
| **Wobbly Life** (`assets/refs/machines.jpg`) | The *environment* target: clean smooth low-poly, candy palette, rounded chunky machinery and vehicles. Bright and toy-like. NOT voxel-gritty. |

**On the term "ultra casual".** Worth being precise, because it is easy to
mis-set expectations with it: *hyper-casual* / *ultra-casual* is primarily a
**market category** — instant-play mobile games with one mechanic — not an art
style. It does carry an associated look (flat colour, primitive shapes, high
contrast, no texture detail) and that look overlaps ours a lot, so it is a fair
shorthand for the *finish*. But it says nothing about the boxiness, which is
the actual rule here. The precise vocabulary for what we are building is
**"chunky low-poly"** or **"blocky / box-modular characters"** — Synty's own
name for the range is the **"Simple"** series. Use those terms in briefs and
asset searches; "ultra casual" will return flat-shaded mobile UI, not box people.

**Cost.** Synty's packs are paid and the whole cast would be a real spend, so
the pipeline reproduces the look from CC0 parts instead: Kenney's *Blocky
Characters* base mesh (6 boxes, 24 verts each, 27 baked animations, CC0) worn
in atlases we paint ourselves. One character = one 1024² PNG, £0. See ART.md
for the pipeline and `tools/gen_blocky_skins.py` for the painter.

### The cast — five rockers, five genre crowds

The story: rock is gone, and the town has been given over to every other genre.
Five rockers are trying to bring it back, one wave at a time. Heroes and enemies
are all ordinary people; what separates them is the music they are dressed in.

| | Heroes (rock) | | Enemies (the waves) |
| --- | --- | --- | --- |
| **Punk** | lead guitar — leather, mohawk, red tee | **Disco** | white three-piece, gold, afro |
| **Metal** | bass — hair to the waist, black on black | **Techno** | head-to-toe black, mirrored cyan visor |
| **Grunge** | drums — red flannel, faded denim | **Hip-Hop** | red puffer, gold chain, baggy denim |
| **Glam** | vocals — purple satin, flares, gold platforms | **Country** | stetson, rust plaid, tooled boots |
| **Rockabilly** | rhythm guitar — bowling shirt, quiff | **J-Pop** | candy twin tails, mint and sky |

Defined once in `src/game/skins.rs` (`BOXY`), painted by
`tools/gen_blocky_skins.py`. See them all:

```bash
cargo run --bin lab -- shot screenshots/compare cmp
```

## Play

```bash
cargo run
```

| command | mode |
| --- | --- |
| `cargo run` | **solo** — the guitarist only |
| `cargo run -- coop` | local **two-player** shared-screen co-op |
| `cargo run -- host` | host a **LAN** game (guitarist); default port 47474 |
| `cargo run -- join <host-ip>` | join a LAN host (drummer) |

The game opens on the **character-select lineup** (`select.rs`) — a stage with
the whole playable cast standing on it, blocky and skeletal side by side, each
with their own waveform turning above their head. `←` `→` choose, Enter
starts; in co-op, Enter confirms P1 then hands the cursor to P2 (Tab swaps back).

Solo raises the stage with the same `Q` build key once the loudspeaker network
is linked (no drummer to do it). LAN co-op (M6) is a prototype pass — see below.

- **P1 — Guitarist** (keyboard + mouse): WASD move · mouse aim · hold LMB fire · RMB Encore blast · Space dodge-roll (i-frames) · `E` get in / out of a car.
- **P2 — Drummer** (gamepad, *or* the keyboard fallback so one person can drive both): left stick / Arrows move · right stick aim · South / `/` drumstick swing · West / `,` beat slam · East / `.` dodge · RightTrigger / `'` get in / out of a car. A 124 BPM click track auto-fires a shockwave pulse off the drummer every beat, bigger on the downbeat.
- **The level loop** (M4): waves come in discrete pulses with a calm **build lull** between them. Kills drop **scrap** into a shared pool. In the lull, plant **loudspeakers** (`Q` guitarist, 18 scrap) — a klaxon on a pole that secures a zone around it (enemies inside pacified, spawn frontier pushed out, music layers in). Loudspeakers **link** when their zones touch; once **3** form one connected network the **stage** unlocks (`;` / gamepad North drummer, 60 scrap) on the **bandstand**. Finishing the stage **wins the level**. Any hero channels any site by standing on it (faster with both).
- **Speakers go on marked pitches** (`Level::speaker_sites`) — ten painted pads around the town, laid out so *adjacent* pitches link and distant ones don't. They show on the minimap, so the build phase is a route you plan rather than a key you press where you stand.
- **Cars can be driven** (`drive.rs`) — `E` next to any parked car. A car crosses the town at more than twice a hero's run and flattens the swarm it drives through; in exchange you can't fire or build from the seat. The **blocky rig sits at the wheel** (Kenney ships a `drive` clip); a skeletal hero is hidden while driving, because nothing in that pack sits down.
- A hero at 0 HP goes **down** (not dead); the other revives them by standing close for ~2 s. Both down → game over. `R` restarts (also from the win screen).
- **V** cycles the experimental camera rigs: Follow (default angled co-op) → Shoulder (street level) → FirstPerson.

## Scale — everything is sized in metres

`src/game/scale.rs` is the single place the world's proportions are decided.
Every spawner used to pick its own multiplier, which is how the town ended up
with **parked cars taller than the band** and **houses a rocker could see
over**: each number looked reasonable on its own. Now a spawner states how big
a thing is *in metres* and `scale::fit(native_height, metres)` turns that into
a factor, using the model's **measured** native height.

```
U_PER_M = 1.28      # a 1.8 m person stands ~2.3 u — the height the heroes already were
PERSON 1.80   GOON 1.88   CAR 1.46   HOUSE 5.4–7.2   TOWER 9–14   SKYSCRAPER 16–20
```

Three things this shook out, all of them real bugs rather than taste:

- **`CITY_MODELS` heights were a unit too large** — the table trusted a comment
  saying the city kit stands at `y = -1`; it stands at `y = 0` like everything
  else. Commercial blocks were being built at roughly half their intended
  height, which is why the middle of town was shorter than the traffic.
- **`skins::BOXY_LIFT` was 1.0** on the belief that `blocky/base.glb` has its
  origin at the hips. It does not — `leg-left` sits at `y = 1.0` with its mesh
  running down to `-1.0`, so feet are already at zero. The entire boxy cast was
  floating a third of a body-height off the ground, including in the contact
  sheets it was being reviewed from.
- **Cars are fitted by height, not length.** Kenney's cars are stubby for their
  height, so matching their length puts a sedan's roof over the band's heads.
  A hero has to be the tallest thing on the street.

Character heights live with the characters, in `skins::Skin::height` /
`skins::Boxy::height`, so "how big is a J-Pop idol next to a metal bassist" is
answered in metres in the table you dress them from.

## Review the art without playing

Validating an art direction by playing the game is slow and unrepeatable. Two
labs do that job instead, and both read the game's own numbers rather than
restating them, so neither can drift.

### Asset lab — characters, skins, animations, props

```bash
cargo run --bin lab
```

One subject at a time under the game's own camera rig, lighting and
gloom→daylight grade, wearing the real per-material tints and face planes.
126 subjects: the **boxy cast** first (the house style — see *Art direction*),
then the Quaternius cast that is parked on `art/quaternius-skeletal`, the vibe
face-cubes, the guitar-gun, then every prop auto-discovered from
`assets/models`.

| key | |
| --- | --- |
| `[` `]` | subject · `Tab` role (stock / guitarist / drummer / gloom goon) |
| `;` `'` `C` | scale multiplier on top of the shipped scale, and reset |
| `,` `.` | animation clip · `Space` pause · `-` `=` speed (0.05–3×) · `0` reset · `L` loop |
| `1`–`7` | 3/4 · front · side · back · top · face · **GAME** (the true in-play distance and angle) |
| `A D W S`, `Q E`, drag, scroll | orbit and zoom |
| `G` `N` `R` | ground swatch · gloom↔daylight · 1 m/1.8 m ruler |
| `B` `M` | carried instrument (flying V / bass / snare / synth / mic stand / none) · how it is worn (slung across the chest / in the aiming hand / stowed on the back) |
| `F` `H` | nudge the face plane / the mounted prop — prints a paste-ready `Transform` |
| `P` `Enter` | print state to stdout · save a screenshot to `screenshots/lab/` |

```bash
cargo run --bin lab -- shot screenshots/lab_sheet        # the whole cast
cargo run --bin lab -- shot screenshots/compare cmp      # boxy vs Quaternius
cargo run --bin lab -- shot screenshots/gear gear        # instruments, carry poses, shot shapes
```

renders offscreen as a contact sheet (no display needed) — the way to review
art from a headless session or diff it between sessions. `cmp` is the narrow
one: the ten boxy characters against the Quaternius skins of the same genre,
which is what `screenshots/compare/SIDE_BY_SIDE*.png` are built from. `gear`
shoots every instrument in every carry pose on one body of each rig — back
poses from behind, hand poses over the shoot clip — plus every candidate
projectile at the true in-play framing.

To judge a projectile from footage rather than a still, `capture` takes the
shot shape as a 4th argument:

```bash
cargo run --release --bin capture -- frames screenshots/run 300 quaver
```

### Audio lab — stems against the beat

```bash
.venv/bin/python3 tools/audio_lab.py && open screenshots/audio_lab.html
```

One self-contained page (stems base64'd in, opens from disk, no server): a
scrubable waveform per stem with speed, gain and loop, a metronome, and the
music bed to layer underneath. Each waveform carries a **beat grid** at the
tempo read out of `drummer.rs`, and each clip reports its length in beats and
how far off-grid it sits.

## Status

**Session 17 (2026-09-04) — pick a character, size the world, shoot sound**

- **Character select** (`select.rs`, `roster.rs`) — the game now opens on a 3D
  lineup rather than dropping you into a run. `roster::Pick` is the one type
  that hides which cast a character comes from; `player.rs` has a single spawn
  path for both rigs, so the **blocky cast is playable**, not just viewable in
  the lab. Clip names are the only rig-specific thing left, and they live in
  `Pick::clip`.
- **The metric standard** (`scale.rs`) — see *Scale* above. Town, cars, houses,
  towers, street dressing, heroes, goons and the face-cube "vibes" are all
  sized from a real-world measurement now.
- **Shots are sound waves** (`weapon::ShotLook::Wave`, now the default) — a run
  of boxes along the flight line, swung off it by the shooter's own waveform,
  with the wave **travelling through the shot while it flies** (sixteen baked
  phases swapped per frame; rebuilding a mesh per bullet per frame would be
  forty uploads a frame). The waveform, pitch, swing and colour come from
  `skins::sound(id)` — so a metal bassist's slow wide saw and a J-Pop idol's
  tight bright sine are visibly different weapons. This is the hook the actual
  audio can hang off later.
- **Loudspeakers go on marked pitches** (`Level::speaker_sites`, `build::Pitch`)
  — ten painted pads laid out so adjacent ones link, shown on the minimap and
  reserved against the building scatter.
- **Drivable cars** (`drive.rs`) — the town's parked cars were scenery; now
  they're transport and a weapon. The blocky rig sits at the wheel.

**Built**
- Bevy 0.19, 3D. Town arena (200×144) with an HDR + bloom + filmic render pass;
  one shared camera (`game::follow_camera`) frames the **two co-op heroes** on
  their midpoint and eases outward as they spread apart, clamped to the arena.
- **Local co-op** — two `Player` entities keyed by `Hero::{Guitarist, Drummer}`.
  Input is funnelled through a per-player `Intent` component by `p1_input`
  (kbd+mouse), `p2_input` (gamepad + keyboard fallback) and `bot_input`
  (capture); everything downstream reads `Intent`/`Aim` only.
- **Downed + revive** (`combat.rs`) — a 0-HP hero goes `Downed` (plays `die`,
  swarm retargets); a team-mate within ~4.4 u for 2.2 s revives them at 45 %.
  Both down → game over.
- Guitarist: move / cursor aim / dodge-roll (barrel-roll, i-frames). Quaternius
  `punk` skeletal mesh, gait machine, black rocker recolour, cyan ground ring +
  key light.
- Guitarist weapon: a red flying-V **guitar-gun clipped into the `Wrist.R` bone**
  of the animated mesh (`player::attach_guns` / `aim_guns`) so it rides the
  animation while the neck stays trained on the aim; fires **spinning guitar
  picks** from the muzzle (`weapon.rs`) with a muzzle flash.
- **Drummer kit** (`drummer.rs`): a 124 BPM `Beat` fires a shockwave ring off
  the drummer every beat (bigger on the downbeat); `Intent::attack` swings a
  drumstick arc (damage + knockback + `attack-melee-right` clip + a wedge VFX);
  `Intent::special` slams a full-size shockwave on a cooldown. Orange skin +
  amber ground ring — reads apart from the guitarist at the top-down camera.
- Pickups (`pickup.rs`, timed spawns + rare kill-drops, ~12 s each):
  **Dual guitar** — a second gun in the left hand, fire from both;
  **Encore** — an on-demand shockwave (RMB) that damages + shoves everything
  the wavefront crosses.
- Enemies: ramping spawner, 3 types — Rusher (fast/fragile), Sponge
  (slow/tanky), Flyer (fast, hovering). Each is one textured **face-cube**
  (`enemy::face_cube` — custom mesh, the painted face upright on every side).
  They spawn in a ring around the player so the big arena stays populated.
- Combat: pick hits, contact damage, kill counter
- **Death pop** (`game/vfx.rs`) — a downed baddie bursts into a shower of chunky
  cube shards (ballistic + tumbling), a white core flash, an expanding emissive
  ground ring and a light punch, all themed to the enemy's hue (BAAM-SQUAD
  style). Fired as an `Explosion` message from `combat::check_death`.
- **Experimental camera modes** (`CameraMode`, cycle with **V**) — `Follow` (the
  shipping angled co-op framing), `Shoulder` (over-the-shoulder near street
  level, down the guitarist's aim), `FirstPerson` (from the guitarist's head).
  The capture bot scripts a cycle through all three.
- **Discrete waves** (`enemy::Wave`) — `Prep` (calm build lull) → `Spawning`
  (a budget poured in as bursts) → `Clearing` (thin the field) → back to `Prep`,
  each wave bigger. HUD shows the phase + lull countdown.
- **The level loop** (`build.rs`) — `Scrap` pool fed by `EnemyDied` (homing
  motes); press-to-place structures paid from it. **Loudspeaker** (guitarist,
  18 scrap, fast channel) — the klaxon GLB; on completion it registers a
  `SecuredZones` zone (radius 19): enemies inside are `Pacified`, `spawn_enemies`
  pushes the ring out of every zone (receding frontier), the music bed fades up,
  and it lays suppressing klaxon fire at the zone edge. Permanent.
  `update_speaker_net` links loudspeakers whose zones overlap (union-find on the
  largest cluster + glowing ground beams). Once `SPEAKERS_TO_LINK` (3) are in
  one network the **Stage** (drummer, 60 scrap, slow channel) unlocks;
  finishing it sets `GameState::Victory`. An unfinished site still suppresses
  nearby spawns.
- **Audio** (`audio.rs`) — synth placeholder OGG stems in `assets/audio/`
  (`tools/gen_audio.py`): stick count-in, power-chord stab (Encore / beat slam /
  stage complete), soft beat click, scrap blip, build clunk, and a 124 BPM
  music loop that `music_bed` crossfades with `SecuredZones::music_level`.
  Gameplay systems fire `AudioCue` messages; `play_cues` turns them into
  one-shots.
- HUD: two hero health bars (with revive %), survival timer, kill + scrap
  counts, **wave number + phase + lull countdown + speakers-linked**, control
  hints, game-over + **win** screens, restart
- **Coverage feedback** (M5): a bottom-right **minimap** (`minimap.rs`) redraws
  a CPU image each frame — town outline + faint road grid, secured-zone glow
  discs (the coverage read, brightening with `music_level`), and live blips for
  the heroes, the swarm, the loudspeakers and the stage. And a **gloom→daylight
  world grade** (`grade.rs`): a `WorldGrade` value eases toward
  `SecuredZones::music_level` and drives the sky / ambient / distance fog / key
  sun, so a music-starved town opens cold and closed-in and lifts into warm
  daylight as the band reclaims ground (same signal the music bed rides).
- Offscreen capture binary for proof video (drives both heroes; under `AutoPlay`
  spawns a denser horde, trickles through the lull, and auto-restarts a few
  seconds after Victory so a long capture keeps showing the loop)
- **LAN co-op (M6, thin-client)** — `game/net.rs`. `shooty host [port]` runs the
  game authoritatively and streams a ~30 Hz UDP snapshot (both heroes, the live
  enemies, secured zones, HUD counters); `shooty join <addr>` runs the same
  binary with every authoritative system switched off and rebuilds that world
  from the stream as "ghost" entities, sending its own drummer `Intent` back.
  Host = guitarist, client = drummer. Plain `UdpSocket`, no netcode crate, no
  matchmaking.
- **Solo / coop / LAN modes** — `shooty` spawns the guitarist alone (`Party::Solo`;
  the guitarist raises the stage with `Q` once the network links); `shooty coop`
  is the local two-hero shared screen; host/join are the LAN game.
- **Person-scale actors** — heroes + enemies are sized against the world now
  (`PLAYER_MODEL_SCALE` 1.0), so buildings / trees / cars read as full-size.
  The **actor-proximity** combat layer was pulled down to match (first pass):
  `PLAYER_RADIUS` 1.9→1.0, enemy hitboxes/visuals ~0.6×, drummer pulse/downbeat/
  slam radii 6.5/10/25→4.2/6.5/15, `MELEE_RANGE` 5.4→3.6, Encore `WAVE_MAX`
  22→14, `REVIVE_REACH` 4.4→3.0, shockwave hit-bands 1.9/1.8→1.2. Structural
  distances (`ARENA`, `SPEAKER_RADIUS`, `STAGE_RADIUS`, spawn rings, move/enemy
  speeds) left for the live tuning pass — see Left.

**Left**
- **Quaternius base-mesh milestone — WIP, in progress.** Hero / drummer / goon
  are on the **Quaternius "Ultimate Modular Men"** CC0 pack now
  (`assets/models/quaternius/`, one 62-bone skeleton + 24 baked skeletal clips);
  reskin is per-material in-engine (`player::hero_tint`, `enemy::gloom_tint`).
  Still open: guitar-gun fit in the `Wrist.R` bone wants tuning; **hair-block
  attach system** not built (Quaternius ships its own hair — may be enough);
  **rect-eye face paint** does not port (Quaternius has a baked face) — decide
  whether that matters; face-cubes now look oversized vs person-scale heroes;
  goons don't play `Death`; `gen_skins.py` is dead; `blocky/` still vendored.
- **People waves** — a humanoid "townsfolk taken by the gloom" enemy alongside
  the cube vibes. The goon path is now that humanoid; still needs its own
  wave/spawn identity + more character variety (`punk`/`worker`/`farmer`/... all
  vendored and share the skeleton).
- **Camera modes are a first pass** — `Shoulder` gets walled in when the hero is
  swarmed; both toys track the guitarist only and want feel-tuning + maybe a
  mode that also frames the drummer in co-op.
- Death pop has no dedicated sfx yet (reuses nothing) — wants a short punchy
  "thock" stem via `tools/gen_audio.py`.
- **Structural distances + feel tuning** — `ARENA` (100×72) still wants
  shrinking so the buildings sit closer; `SPEAKER_RADIUS` / `STAGE_RADIUS` /
  the spawn rings / move + enemy speeds are all still first-guess against the
  person-scale hero. Needs a live pass (feel, not just looks); the
  actor-proximity numbers above may also want a nudge once played.
- **M6 is a prototype pass** — position/health/state/zones/waves replicate;
  bullets, pickups, drum & Encore VFX and audio cues do **not** cross the wire
  yet (the client sees hits land via the shared kill/scrap counters and the
  enemies vanishing, but not the projectiles). No interpolation/prediction
  beyond a soft position correction on the client's own hero, no reconnect, no
  in-game join UI. One client only.
- **Balance pass, still owed** — the M4 discrete-wave cadence is in, but the
  per-wave budgets / scrap costs / channel times are first-guess numbers.
  Guitarist pickups + drummer beat AoE + loudspeaker fire still over-clear
  (the person-scale AoE shrink helps but the wave budgets are untouched).
  Pickups always buff the guitarist regardless of who grabs them.
- The win is a text screen — no band-onstage sequence yet (deferred in ART.md).
- The Encore/stage still want the count-in to actually *lead* the chord musically.
- Goon cigarette prop / death clip; green rusher vs green grass still a touch
  camouflaged; a couple of terracotta suburban houses read as furniture top-down.
- **M6 next steps** (if the vibe warrants going further): replicate bullets +
  VFX + audio cues, add client-side interpolation, a join screen, reconnect.

## Resume here (2026-09-03) — M1–M6 all in

Co-op rework: **M1** (graphics + town), **M2** (local co-op + drummer),
**M3** (build phase + audio), **M4** (the level loop), **M5** (coverage
feedback) and **M6** (LAN co-op, thin-client) are all landed. A balance pass is
still owed, and M6 is a deliberately-minimal first pass — see **Left** above and
the memory scope note.

M6 this session — **LAN co-op (thin snapshot client)**:
- `game/net.rs` (`NetPlugin`) — `NetRole::{Local,Host,Client}` resource +
  `authoritative` / `is_host` / `is_client` run-conditions. `launch_from_args`
  parses `host [port]` / `join <addr>`; `lib::run` adds the plugin.
- **Host**: `host_recv` (drain the client's `InputFrame`, OR-accumulate edge
  actions), `assign_net_ids` (tag fresh enemies with a `NetId`),
  `apply_remote_intent` (write the client's frame onto the host's drummer
  `Intent`/`Aim`, then clear edges), `host_send` (PostUpdate, ~30 Hz — pack a
  `Snapshot` and `send_to` the learned peer).
- **Client**: every authoritative system set now carries
  `.run_if(super::net::authoritative)` (enemy `run_wave`/`move_enemies`, combat,
  build, drummer, pickups, weapons, hud `restart`); `p1_input` early-returns on
  a client, `p2_input` early-returns on a host. `client_recv` keeps the newest
  `Snapshot`; `apply_snapshot_state` (Update, **not** state-gated so it can
  follow a host restart) writes `Score`/`Scrap`/`RunClock`/`Wave`/`SpeakerNet`/
  `SecuredZones` + drives `GameState`; `apply_snapshot_transforms` (PostUpdate
  before `TransformSystems::Propagate`) hard-drives the ghost guitarist and
  soft-corrects the predicted drummer; `enemy::net_apply_enemies` reconciles
  ghost enemies by `NetId` (shared `spawn_enemy` helper, extracted from
  `run_wave`); `build::net_apply_zones` raises a loudspeaker/stage ghost per new
  secured zone (`Wave::net_apply` added for the HUD).
- Wire types are plain `#[derive(Serialize,Deserialize)]` structs (`serde` +
  `bincode`, new deps) with `[f32;N]` fields — no glam-serde dependency.
- Verified: handshake + snapshot round-trip over `127.0.0.1` via headless logs
  (`net: client connected` / `net: receiving snapshots`). **Not** yet verified
  rendered — no display in the build env; needs a real two-machine run.

M5 this session — **coverage feedback**:
- `game/minimap.rs` (`MinimapPlugin`) — a 192×138 `Rgba8UnormSrgb` image
  (`MinimapTex`, built once, reused across runs) redrawn every frame by
  `draw_minimap`: grass/road base via `Level::on_road`, secured-zone glow discs
  scaled + brightened by `music_level`, then blips for enemies, build sites
  (pulsing), loudspeakers, stage and the two heroes. Shown bottom-right in a
  framed `ImageNode` (`ImageSampler::nearest`).
- `game/grade.rs` (`GradePlugin`) — `WorldGrade(f32)` eases toward
  `SecuredZones::music_level() / 0.95`; `lift_the_gloom` smoothersteps it and
  lerps `ClearColor`, `GlobalAmbientLight`, the camera `DistanceFog` and the
  `KeySun` `DirectionalLight` between a `GLOOM` and a `LIT` endpoint. `KeySun`
  marker added to the warm sun in `lib.rs::spawn_key_light` (both bins).
- Proof video → `screenshots/m5/shooty_m5_coverage.mp4`.

M4 — **the level loop**:
- `build.rs` reworked. `Amp` → `Speaker` (the klaxon GLB), permanent, secures a
  `SecuredZones` zone (`Zone { center, radius }`). `SpeakerNet` +
  `update_speaker_net` (union-find) tracks the largest linked cluster and draws
  ground beams. `place_structures` gates the stage on `net.largest >=
  SPEAKERS_TO_LINK`; `channel_sites` sets `GameState::Victory` when the stage
  finishes. `on_speaker_ready` drives `emissive` off the klaxon's baked atlas.
- `enemy.rs` — `Spawner` (continuous) → `Wave` state machine
  (`Prep`/`Spawning`/`Clearing`), `wave_shape(number, capture)` budgets. A
  capture-only lull trickle keeps the proof video alive.
- `GameState::Victory` (`mod.rs`); `hud.rs` wave/phase/linked readout +
  `spawn_victory` screen (R restarts). `bot_input` walks a fixed loudspeaker
  triangle then raises the stage. `capture.rs` `auto_restart` loops it.
- Klaxon model: `assets/models/klaxon.glb` (Grok ref → Tripo3D, ~32¢).
- Proof video → `screenshots/m4/shooty_m4_levelloop.mp4`
  (`tools/encode_video.py`).

M3 — **build phase + audio**:
- `build.rs` — `Scrap` pool (homing `ScrapMote`s minted from `EnemyDied`, which
  now carries `kind`). `place_structures` drops a `BuildSite` at the pressing
  hero's feet; `channel_sites` fills it while heroes stand in `CHANNEL_REACH`.
  `Amp` (guitarist) → `amp_fire` bolts + `Lifetime`. `Stage` (drummer) →
  `SecuredZones` entry + a two-tier deck. `pacify_zone` tags enemies inside as
  `Pacified`; `enemy::move_enemies` drifts them out; `enemy::spawn_enemies`
  `push_out`s the ring (frontier) and slows near unfinished sites (lull).
- `audio.rs` — `AudioCue` message + `play_cues` one-shots + `music_bed`
  crossfade. Stems: `tools/gen_audio.py` → `assets/audio/*.ogg` (synth
  placeholders; `soundfile` added to `.venv` to encode Vorbis).
- `Intent.build`; `hud.rs` scrap counter; `bot_input` places/channels
  structures and roams toward unsecured ground.
- Proof video → `screenshots/m3/shooty_m3_build.mp4` (M2's is
  `screenshots/m2/shooty_m2_coop.mp4`).

Capture recipe: `cargo run --bin capture -- frames screenshots/<dir>/frames 900`,
eyeball frames, then
`.venv/bin/python3 tools/encode_video.py screenshots/<dir>/frames out.mp4 30`
(ffmpeg isn't installed; `imageio-ffmpeg` bundles a binary).

Tuning knobs: `ARENA` / `CAMERA_OFFSET` / `CAMERA_SPREAD_ZOOM*` in `game/mod.rs`,
camera follow/clamp in `follow_camera`; hero HP + spawn points + `MOVE_SPEED` in
`player.rs`; drummer `BPM` / `PULSE_*` / `SLAM_*` / `MELEE_*` in `drummer.rs`;
enemy `stats()` + `batch`/`interval`/cap (with the `capture` split) in
`enemy.rs`; `FIRE_RATE`/`BULLET_DAMAGE` in `weapon.rs`; `BUFF_SECS` / wave
consts in `pickup.rs`; `AMP_*` / `STAGE_*` / `*_COST` / `*_CHANNEL` /
`STAGE_RADIUS` / `LULL_RADIUS` in `build.rs`; `BPM` + cue volumes in
`audio.rs`; `GLOOM` / `LIT` endpoints + ease rate in `grade.rs`; `MW` / `MH`
+ blip sizes in `minimap.rs`; bot steer + build triggers in
`player.rs::bot_input`.

## Assets

See `ART.md` for the full art direction and pipeline. Current in-game assets:

| What | How | Path |
| --- | --- | --- |
| Hero / drummer / goon mesh + anims | Quaternius "Ultimate Modular Men" CC0 — one 62-bone skeleton, 24 baked skeletal clips, 11 characters | `assets/models/quaternius/` |
| Hero / drummer / goon skins | in-engine per-material recolour by `GltfMaterialName` (`player::hero_tint`, `enemy::gloom_tint`) — no texture | — |
| Enemy faces | `tools/gen_faces.py` — painted PNG on a solid-hue cube | `assets/models/faces/` |
| Floor grid | `assets/textures/floor.png`, repeat sampler + `uv_transform` | `assets/textures/` |
| Guitar-gun, picks, pickups, sound-wave | Bevy mesh primitives (built in code) | — |
| Town buildings (commercial) | Kenney "City Kit (Commercial)" CC0, colormap-tinted | `assets/models/city/` |
| Town houses (suburban) + fences + planters | Kenney "City Kit (Suburban)" CC0 | `assets/models/suburban/` |
| Parked cars | Kenney "Car Kit" CC0 | `assets/models/cars/` |
| Lamp posts / road signs | Kenney "City Kit (Roads)" CC0 | `assets/models/city-roads/` |
| Benches / bins | Kenney "Furniture Kit" CC0 | `assets/models/furniture/` |
| Trees / rocks / bushes | Kenney "Nature Kit" CC0, hue-routed to foliage/bark mats | `assets/models/nature/` |
| Pavements, crossings, plaza, fountain, bandstand, pond | Bevy mesh primitives (`level/terrain.rs` + `level/town.rs`) | — |
| Stage / scrap motes / pacify pools / link beams | Bevy mesh primitives (built in `build.rs`) | — |
| Klaxon loudspeaker (M4 per-zone "secure" structure) | Grok ref (`raw/klaxon.png`) → Tripo3D GLB, ~4.6m tall in-game. `build::on_speaker_ready` drives `emissive` off the baked atlas so the cyan horn mouths bloom. | `assets/models/klaxon.glb` |
| Audio stems (count-in, chord, beat, scrap, build, music loop) | `tools/gen_audio.py` — numpy synth → OGG | `assets/audio/` |

Reference images for future art: xAI Grok (`tools/gen_image.py`), ~2¢ each.
Abandoned Tripo3D image-to-GLB models (`player/rusher/sponge/flyer.glb`) are kept
in `assets/models/` but unwired — see ART.md "Abandoned". Pinterest refs in
`assets/refs/`, old 2D sprites in `assets/sprites/`.

## Layout

- `src/lib.rs` — app, window, 3D camera + light (`KeySun` marker), state wiring
- `src/game/mod.rs` — `ground()`/`plane()` coord helpers, `Hero`/`Downed`/`nearest_player`, party `follow_camera`, shared components, teardown
- `src/game/level/` — **where a run happens.** `mod.rs` (the `Level` record, `LevelId`, the `CurrentLevel` resource, the shared `Ctx`/`Palette`/`Mats`), `kit.rs` (the three ways to put a kit model on the ground), `terrain.rs` (ground slab, road grid, pavements, crossings, perimeter — all from data), `town.rs` (the shipping level)
- `src/game/{player,drummer,weapon,enemy,combat,pickup,build,audio,hud,minimap,grade}.rs` — feature plugins (`minimap` = corner coverage map, `grade` = gloom→daylight world lift)
- `tools/gen_faces.py` / `tools/gen_audio.py` — enemy face-cube textures, audio stems
- `tools/gen_people.py` — non-engine face/hair/limb art-direction sheets (`screenshots/people/`)
- `tools/gen_skins.py` — **dead** (Kenney atlas remap; superseded by in-engine per-material recolour)
- `src/bin/capture.rs` — offscreen capture; freezes the sim during a pre-roll so GLBs finish loading
- `src/bin/lab.rs` — the asset lab (above); borrows `player::hero_tint`, `enemy::gloom_tint`, `enemy::face_placements`, `grade::apply_grade` so it renders what the game renders
- `tools/audio_lab.py` — the audio lab (above); reads BPM from `drummer.rs`
- `tools/gen_image.py` — xAI Grok image generation
- `tools/chroma_matte.py` — (legacy 2D) chroma-key matting

## Resume here (2026-09-04, evening) — bad vibes have a source, and the music comes back

**The idea.** The town's music has gone and five people are taking it badly.
Waves no longer spawn in a ring around the party — they pour out of *those five*.
Clear what one of them has put on the street and they snap out of it: colour
floods back, they walk to the bandstand, and **their stem of the soundtrack
joins the mix**. Cheer up all five and the whole track is playing.

### What is on disk

- **`src/game/gloom.rs`** (new) — the whole mechanic. `GloomSource` + `Mood`
  (`Dormant → Brooding → Marching → Happy`), `FromGloom(usize)` on every enemy,
  and `Chorus { level: [f32; 5], cured }`, which is the one thing that crosses
  from gameplay into the mixer.
  - **Cure rule:** this source has had crowd alive at some point (`seen`) and
    now has none. No phase check, no timer. Cures can therefore land *mid-wave*.
    The `seen` latch exists because enemy spawns go through `Commands` and so do
    not exist until the next flush — without it a source cures itself for free
    on the frame its first batch is ordered.
  - `CAST` is the wake order **and** the arrangement order: hiphop (kit) →
    techno (bass) → disco (chords) → country (hook) → jpop (topline).
- **`assets/audio/stems/*.ogg`** + **`tools/gen_stems.py`** — five loops of one
  124 BPM / A-minor / 8-bar track, byte-identical in length. Each is written to
  hold up *alone* and to work in any of the 31 subsets, since the player picks
  the order. Separated by register and by where they sit in the bar.
- **`src/game/audio.rs`** — rewritten. The single `music_loop.ogg` bed is gone
  (two pieces of music fighting is worse than either). `start_stems` waits until
  **all five** stems have loaded and starts them on one frame; equal length +
  one start = phase-locked for the run, and after that only volume ever changes.
- **`enemy.rs::run_wave`** — pours from the **newest-woken brooding source and
  only that one**. Splitting a wave across every vent sounds fairer and is much
  worse: each source is topped up as fast as it is cleared, none ever reaches
  zero, and the cure never fires. Verified: with the split, 1 cure in 80s; with
  one vent, 4 cures in 90s in wake order.
- **Art (user direction, same session):** bad vibes are now **dark heads with
  glowing faces**, not saturated cubes — `tools/gen_faces.py` paints near-black
  heads whose eyes/mouth are the only lit thing, and the same PNG is the
  base-colour *and* emissive map. Expressions are feelings, not monsters:
  anxious / despairing / bitter. `vibe_materials()` in `enemy.rs` is now shared
  with `lab.rs`, which used to build its own copy and could drift.
- **Goons are out of the mix** (`goon_chance = 0.0`). The humanoid tier read as
  a different game spliced into the swarm. Code left in place — the net layer
  still reconciles against `spawn_goon`.
- **`AudioCue::Pop`** on every enemy death (`tools/gen_audio.py::make_pop`):
  dry, tiny, randomly pitched 0.84–1.24×, capped at 4 per frame so a drummer
  slam is popcorn rather than one flanged thud.
- **`cargo run --bin capture -- gloom <dir> <n>`** — new capture mode that
  frames the *mechanic* (the live vent, then whoever is walking to the
  bandstand) instead of the hero. The hero-follow camera never shows either.

### Traps this session cost time on — do not re-discover

- **Bevy runs the initial `StateTransition` before `Startup`.** A binary that
  opens directly in `Playing` (the capture bot) fires `OnEnter(Playing)` first,
  so anything a Startup system was meant to have created is missing. Stem
  handles are built in `setup_audio` for exactly this reason.
- **libsndfile segfaults writing long OGG/Vorbis files.** The 15s stems are
  fine; a 170s preview crashed mid-write. Write WAV and convert with `afconvert`
  (macOS built-in) — `ffmpeg` is only available via `imageio_ffmpeg`.
- **The town's PARK tree ring is placed directly, not through `solids`,** so it is
  the one piece of town dressing that ignores a reserved site. The first hiphop
  site was invisible behind three of its trees.
- **The face quad's `flat_top` needed an extra `-FRAC_PI_2` about Y.** Laying it
  flat alone leaves the face a quarter turn round when seen from above —
  invisible while these were plain cubes, glaring now the face is the character.

### Open / next

- **Only 4 of 5 cure in a 90s capture** — jpop (the last-woken) rarely gets
  there. Either shorten the wave cadence or wake sources faster.
- **Nothing marks "all five" yet.** `Chorus::complete()` exists and the HUD
  prints `THE BAND IS BACK`, but no state change hangs off it; the existing
  `Victory` is still the stage-building one.
- **The cured crowd stands a bit spread and a tree occludes part of the ring**
  at the bandstand. Slots are `STAGE_RING = 6.5` in `gloom.rs`.
- **Gloom sources are not net-replicated.** On a client they stand there and
  never cure (`seen` is only set by the authoritative spawner's enemies).
- **Stems are placeholder-grade synth.** If they get replaced: generate five
  *separate single-instrument* pieces sharing tempo/key, **not** one track put
  through stem separation — a separated "other" bus heard alone sounds gutted,
  and the player hears arbitrary subsets. ElevenLabs Music has a documented API
  with stem output; Stable Audio 3.0 is the most stems-oriented model; Suno and
  Udio still have no official public API as of mid-2026.

## Resume here (2026-09-05) — a level is data now

**The idea.** Before this pass the game had exactly one place in it, and it was
not a *thing* — it was a thousand lines of `env.rs` with the road positions, the
arena size, the loudspeaker pitches and the five gloom sites all written out as
module constants beside the code that spawned them. Fine for one town, and a
dead end for a second: every question another environment would ask ("how big is
this?", "where can a speaker go?", "is that tarmac?") had a single hard-coded
answer, in a private module, reachable only through shims.

So a place is a **`Level`** now: a plain record of what it is, plus one function
for what only it does.

### What is on disk

- **`src/game/level/mod.rs`** — `Level` (name, arena, palette, terrain, speaker
  pitches, stage site, gloom sites, `dress`), the `LevelId` enum, the
  **`CurrentLevel` resource**, and `Ctx` — the one bag a `dress` function gets:
  world access, the level, an rng, the ground-reservation list, shared primitive
  meshes and the palette realised as materials.
- **`src/game/level/kit.rs`** — the three ways to put a model on the ground
  (`building` / `prop` / `nature`) plus `colormap()`. This is why adding a Kenney
  kit is a table of paths and a colormap handle, not new code.
- **`src/game/level/terrain.rs`** — ground slab, road grid, kerbs, pavements,
  crossings, perimeter, all driven from `Terrain` + `Palette`. A level with no
  road grid leaves `Terrain::roads` `None`.
- **`src/game/level/town.rs`** — the shipping town, now expressed on top of all
  of that. Mostly tables.
- **`src/game/env.rs`** — gone.

### What changed elsewhere

- **`ARENA` is deleted.** Seven modules read it; they now read `level.arena` off
  `Res<CurrentLevel>` (camera clamp, swarm spawn ring, car and pickup bounds,
  player wander and movement clamp, minimap projection).
- **`gloom::CAST` split in two.** `gloom::STEMS` keeps the part that is a game
  rule — who the five are and the order they wake in — and *where they stand*
  moved to `Level::gloom_sites`, index for index.
- **`gloom::bandstand()` is gone**; `level.stage_site` is public, so the capture
  binary reads it directly instead of through a shim.
- **`SHOOTY_LEVEL=<name>`** picks the environment at launch. There is no picker
  in the UI and argv belongs to the networking modes, so this is how you boot a
  level that is still being built. Unknown names warn and fall back.

### Proof

Captured 120 frames and compared them against `screenshots/gloom/` from before
the refactor: same town, frame for frame. The refactor is behaviour-preserving —
`cargo clippy --all-targets` adds no new warnings.

### Open / next

- **No second level yet.** The abstraction is unproven until something that is
  *not* a 4×4 road grid goes through it — the likely first stress is an airfield
  or a ski town, where the terrain is an apron or a slope rather than streets.
- **`Palette` may be the wrong seam.** It currently carries both terrain colours
  and dressing colours; if a level wants snow-white pavements but timber-brown
  benches it works, but if two levels want genuinely different material *roles*
  the shared `Mats` will need splitting.
- **No level picker.** `select.rs` chooses characters; nothing chooses a place.
- **Asset shortlist for new environments** (researched 2026-09-05, all CC0
  unless noted): Kenney has ten more kits in the family already in use —
  Modular Buildings (100), Retro Urban (120), Building Kit (80), City Kit
  Industrial (40), Factory Kit (140), **Holiday Kit (100, the snow/ski base:
  snow-roofed cabins, snowy conifers, drifts, sleds)**, Space Kit (150), Space
  Station Kit (90), Train Kit (100), 3D Road Tiles (300). Quaternius' Toon
  Shooter Game Kit (73, CC0) is the best of the poly.pizza bundles — ~45
  environment props, no buildings, and a visibly different house style. Gaps
  with no CC0 source: **aircraft, snowmobile, chairlift** — Tripo3D hero props,
  ~32¢ each, the klaxon route.

## Resume here (2026-09-05, later) — one world, with districts in it

**The idea changed mid-session.** The plan had been three or four separate
environments; what the user actually wants is *one linked map* with districts —
downtown, suburbs, nature, industry, and an airfield in the corner — Wobbly Life
style, so the same world can be reused for several games and for experimenting
with vehicles and flying. Space stays a **separate world**, and when it is built
**only the hero characters carry over** — enemies, vibes, props, terrain are all
new assets (the Quaternius space packs).

The `Level` abstraction survived that turn intact, which was the point of doing
it first: "a level" simply became "a world", and `LevelId` grew a second entry.

### What is on disk

- **`src/game/level/world.rs`** — the world. Arena 600 × 448 u (469 × 350 m,
  nine times the town). Downtown in the middle, suburbs west and north-east, an
  industrial estate south-east, an airfield along the north edge with a 170 m
  runway, apron, taxiway and hangars, and woods and farmland filling the
  south-west. ~620 placed things; builds in well under a second.
- **`src/game/level/kits.rs`** (new) — the kit tables and colormap materials,
  moved out of `town.rs`. These describe the *kits*, not any one level, so both
  levels share them and a third costs nothing.
- **`src/game/level/terrain.rs`** — **roads are segments now**, not a grid. A
  `Road` has a start, an end and a `Surface` (`Street` or `Runway`); crossings
  are painted only where two streets genuinely overlap. The old two-lists-of-
  centre-lines version could only ever describe a town.
- **`tools/measure_models.py`** (new) — prints measured native heights straight
  from the glTF POSITION accessors. Validated against every hand-measured value
  already in the tables; it reproduces all of them exactly.
- **`src/bin/capture.rs`** — new **`map`** mode: a high orbiting camera that
  frames a whole level, sized from the level's own arena. The play camera sits
  20 units up and frames two people; it is the wrong instrument for judging a
  470 m world, and this was the difference between guessing and seeing.

### Assets installed (all CC0, all refreshed to current versions)

City Kit Commercial (41), Suburban (40), **Industrial (37, new)**, Roads (95,
was 28), Car Kit (50, was 18), Nature Kit (329, was 11), Furniture (140, was 9).
Every filename the code already referenced survives in the new versions, so the
refresh was safe. The roads kit turns out to be the important one: modular
tiles with a `-barrier` variant of nearly everything, which is the district
gating mechanism when we want it.

### Two bugs the world exposed

- **The sandbox was permanently dark.** `grade::lift_the_gloom` grades the world
  from `SecuredZones::music_level`, which can only rise by curing gloom sources.
  A level with none sat at maximum gloom forever. It now treats an empty
  `gloom_sites` as "no reclaim arc — this is daylight".
- **Fog was tuned for a town.** Linear 95→210 units fogged out the next district
  entirely. `Level::fog_scale()` stretches it with the arena; the town scales by
  exactly 1.0 so nothing about it changes.

### Open / next

- **No aircraft.** Still no CC0 low-poly plane — the runway and apron are built
  and empty. Tripo3D hero prop, ~32¢, the klaxon route.
- **Flying is not implemented.** The world is drivable (`E` next to any car);
  flight is its own feature — altitude, a camera that copes with it, and the
  arena clamp is currently 2D.
- **District gating is not implemented.** `world::district(p)` exists and names
  the district a point is in; nothing consumes it yet.
- **Farm buildings.** Quaternius' Farm Buildings pack (13 models, CC0) is a good
  match but ships **FBX/OBJ/Blend only, no glTF**, and is vertex-coloured rather
  than atlased — so it needs a conversion step and will not use the colormap
  path. The fields, fences and crop rows are all Kenney nature kit and are built;
  the barn and silo drop in on top later.
- **The woods read thin.** 143 conifers at packing radius 2.4; a real wood wants
  either more trees or bigger ones.
- **`assets/models/city/screenshots/` is 214 MB of stray capture output** from an
  earlier session, sitting inside `assets/`. Untracked, so it costs nothing in
  git, but it does not belong there.

### Fixes after the first play (2026-09-05)

- **Cars drove at an angle to where they pointed.** `kit::prop` baked the spawn
  yaw onto the *mesh child*, but `drive::steer` reads and writes the **anchor's**
  rotation and seats the driver off it. A car spawned facing east therefore drove
  north while looking east, which reads as swinging around a fulcrum somewhere
  off in the next street. Yaw now goes on the anchor in `kit::prop`,
  `kit::prop_scaled`, `kit::building` and `kit::nature`. **This bug was in the
  town too** — it only became obvious in a world with many east-west streets.
  (The car *models* are fine: all 50 measure dead-centre.)
- **Cars all parked on the first two streets.** `parked_cars` walked the network
  in order and stopped at the cap. It now collects every legal kerbside slot,
  skips junctions, shuffles, and takes 64 off the top — same budget, whole map.
  They also nose with the traffic instead of all facing one way.
- **Houses overlapped the pavement.** `frontage` took a flat setback measured to
  the building's *centre*, so anything wide hung over the kerb. It now picks the
  model first and derives the setback from `ROAD_HW + WALK_HW*2 + its own
  radius + a verge`.
- **Benches were 0.72 m long; rocks were 5.5 m across.** Both were being fitted
  by **height**. The bench model is 0.40 long × 0.47 tall — a ratio no uniform
  scale can turn into a bench — so `scale::fit3` now fits it per axis to
  1.55 × 0.85 × 0.62 m. The bushes and rocks are flat and wide, so `SHRUBS` now
  stores each model's **widest** native dimension and `scale::SHRUB` means
  "across", not "tall".

### Aircraft — a licensing decision to make

All three candidates are **CC-BY, not CC0**: "Plane 2" (Jake Blakeley), the
small airplane (Vojtěch Balák), and the Blimp (Poly by Google). Everything in
this project is currently CC0 with no attribution obligation. CC-BY is fine to
use, but it needs a visible credit — a credits screen or a line in the HUD/README
— so it is worth deciding deliberately rather than drifting into it.

### Rebuild: the network lays out first (2026-09-05)

The first world hand-wrote a road list and scattered buildings near it. From the
air that produced streets stopping in the middle of fields, a rigid symmetric
grid, and buildings standing on pavements. All three are gone, and the order is
now inverted: **the network is generated, and everything else is derived from
it.**

- **`src/game/level/net.rs`** (new) — `Net` (every road, plus the questions
  asked of it), `Block`, `subdivide`, `blocks_of`. Local streets come from
  **recursive subdivision**: split a district with a street that runs wall to
  wall, split each half, stop at the district's own `min_block`. Every street
  therefore ends on another street *by construction* — there is no connectivity
  pass because there is nothing to connect. Splits are deliberately off-centre
  (0.34–0.66), which is what stops it looking like graph paper.
- **The skeleton is irregular and asymmetric.** Arterials sit at uneven
  spacings, and several stop on another arterial rather than crossing it — a
  T-junction reads far less like a grid than a crossroads.
- **`Level.roads` is a generator**, not a list, and `Level.seed` is fixed, so a
  world is the *same* world every load. A place you drive around and learn
  cannot reshuffle each run.
- **Buildings fill blocks, they do not hug roads.** `fill_blocks` insets each
  block by the paving's half-width plus the building's own radius, so standing
  on a pavement is geometrically impossible rather than caught by a check.
- **`Net::paved_clearance` replaced `on_road` for placement.** `on_road` asks
  "is this point on tarmac", which says nothing about where a building's *edges*
  land, and was the direct cause of the pavement overlaps.

Three measurements drove the tuning, all from diagnostics rather than guesses:

| Symptom | What the counters said | Fix |
|---|---|---|
| Downtown nearly empty | 57 of 150 placements collided | Towers have ~9 u radius; the plot pitch was 11–17 |
| Blocks placed nothing | 8 blocks offered **5** cells | Inset used the *worst-case* radius; one short-wide model (0.89 tall × 1.33 wide) became 32 m across |
| Suburbs fell 40 → 18 | — | Inset is paid on all four sides, so **fewer bigger blocks fit more** than many small ones. Shrinking `min_block` was backwards |

### Farmland — Quaternius Farm Buildings, converted

13 CC0 models (barn, big barn, open/small barn, silo, silo house, chicken coop,
water tower, two windmills, well, two fences). The pack ships **OBJ only**, from
a Google Drive folder, so they were converted with `trimesh`
(`.venv/bin/pip install --index https://pypi.org/pypi trimesh`). They keep their
own materials — spawn with `kit::model`, not `kit::prop`.

**They are authored in metres**, unlike everything else here, so they scale by
`U_PER_M` flat: a barn measures 6.01 units and is a 6 m barn.

### Aircraft in

"plane 2" and "Small Airplane" on the apron, and the Blimp drifting a slow lap
over the world (`level/sky.rs`). All three are **CC-BY** — see
`assets/models/aircraft/LICENSE-cc-by.txt`, which carries the exact credit line.

### Open / next

- **The credits screen does not exist yet**, and the CC-BY aircraft need it
  before any public build. The attribution currently lives only in that licence
  file and here.
- **Flying is still not implemented.** The plane is scenery.
- **Density is uneven** — downtown carries ~20 towers where it could hold more,
  and some blocks are still bare. The levers are `min_block` per district and
  the plot `gap`; the diagnostics behind `debug!` in `fill_blocks` report cells
  vs placed vs why-rejected.
- **District gating** still unbuilt; `world::district_at` names the district.

## Flight and credits (2026-09-05)

### Flying

`E` next to the aircraft on the apron gets you in. **`W`/`S` throttle ·
`A`/`D` turn · `Space` climb · `LeftShift` descend · `E` out** (only once you
are back on the ground and slow — there is no parachute).

`src/game/fly.rs` is an arcade model in three numbers: throttle, airspeed,
altitude. The one piece of real aerodynamics kept is that **you cannot climb
below `TAKEOFF` airspeed, and if you let the speed fall off in the air you
sink whatever the stick says** — which is what makes taking off and landing feel
like anything. Turn rate scales with airspeed, and the plane banks and pitches
into what it is doing (cosmetic, but a plane that turns flat looks like a car
with wings).

It deliberately mirrors `drive.rs`: same interact key, same mount/dismount
shape, same "the vehicle carries its pilot" rule.

Two supporting changes:
- **The follow camera rises with the pilot** (`drive_camera`). It had a fixed
  ground offset, so a plane at altitude simply left the frame.
- **`Intent` gained `climb`**, rather than `fly.rs` reading the keyboard
  directly, so the flight path is scriptable — which is what let the take-off be
  *verified* rather than asserted.

**Verified**: `cargo run --bin capture -- fly <dir> <frames>` scripts a take-off
by driving `Intent` — the same channel a keyboard writes to. Telemetry (behind
`RUST_LOG=shooty=debug`) shows it accelerate through `TAKEOFF` (30), rotate, and
climb to the 150-unit ceiling at 74 u/s.

### Credits — and a font bug it exposed

`C` on the character-select screen. `src/game/credits.rs`.

The aircraft block is drawn first and brightest because it is the part that is
**legally required**; the CC0 acknowledgements below it are courtesy.

**Bevy's built-in font has no glyphs beyond roughly ASCII.** The select screen
had been quietly drawing `·` and `←` `→` as empty boxes for some time, and the
credits screen could not have used it at all: "Vojtěch Balák" would have come
out mangled, which is a **failed attribution**, not a cosmetic bug. So
`assets/fonts/DejaVuSansMono.ttf` is now bundled (monospace, matching the HUD;
covers Latin Extended-A; freely redistributable, licence beside it) and used by
`ui::font` for the credits and the select screen. The boxes are gone from both.

### Open / next

- **The HUD still uses the default font.** It happens not to contain any glyph
  outside the default's range today, so nothing is broken — but it will be the
  moment someone writes a `·` into it. Pointing it at `ui::font` is a one-liner.
- **No landing challenge.** You can put the plane down anywhere flat; there is
  no requirement to use the runway, and nothing bad happens if you fly into a
  tower.
- **Flight is single-seat and local.** `Intent::climb` is not replicated, so a
  net client cannot fly.
- Density in the world is still uneven; district gating still unbuilt.

## Resume here (2026-09-06) — a wave is dark moods, not boxes

The face-cube enemies (`Rusher`/`Sponge`/`Flyer` + the disabled humanoid
`Goon`) are gone. A wave is now **drained, glowing-faced clones of the citizen
it spilled out of** — you are fighting a shadow of the disco crowd, not the
disco crowd. Direction from the user: use the animated characters we already
have, make it clear the musicians aren't killing the townsfolk, and let some
enemies shoot back.

### What is on disk (uncommitted)

- **`EnemyKind`** (`mod.rs`) → `Mood` · `Head` · `Heckler` · `Sink`. All four
  are the boxy `blocky/base.glb` rig — the same rig the heroes and the five
  gloom townsfolk stand on — so nothing new was generated or imported.
  - **Mood** — a walking clone, 1.42 m (a head shorter than the 1.8 m citizen).
    The bulk of a wave.
  - **Head** — just the head, detached, bobbing and tumbling. Fast, fragile.
    This is the old face-cube, re-dressed: dark cube + a glowing `FACE_STYLES`
    expression (`anxious` / `despairing` / `bitter`, was rusher/sponge/flyer).
  - **Heckler** — closes only to `HECKLER_STANDOFF` (13 m), then holds and
    throws a dark **bad-vibe bolt** (`EnemyBullet`, a new component kept
    separate from `Bullet` so hero and enemy fire never cross-hit).
  - **Sink** — oversized (2.5 m), soaks fire, melee. The one tier allowed to
    loom over a hero (ART.md "A hero is the tallest thing on the street").
- **`enemy.rs`** rewritten around this. `EnemyModel { kind, genre }` on the
  model-root child; `on_enemy_ready` paints every part with the drained genre
  atlas (`dark_skin[genre]` — genre atlas × near-black base colour) and hangs a
  genre-hued glowing face panel over the head, then loops `walk` (Mood/Sink) or
  `holding-right` (Heckler). `genre` is the `gloom::FromGloom` index, so a wave
  from the hip-hop source wears drained hip-hop and glows red.
- **`combat.rs`** — new `enemy_bullets_hit_player` (mirrors
  `bullets_hit_enemies`; ignored while a hero is rolling or driving).
- **`enemy.rs`** — `heckler_fire` + `move_enemy_bullets` in the authoritative
  set. Verified firing in a `gloom` capture (15 shots in 260 frames).
- **`vfx.rs`** death-pop hues → ashen violets / dark rose.
- **`tools/gen_faces.py`** + the 3 PNGs renamed `anxious`/`despairing`/`bitter`.
- **`src/bin/lab.rs`** — `Subject::Vibe(usize)` is now a head expression index.

### Verified

`cargo check`/`clippy` clean (both bins). `cargo run --bin capture -- gloom`
shows the crowd reading as dark walking people with wrong faces, heads mixed in,
pouring from the vent — `screenshots/` not kept; regenerate with that command.
Proof clip sent to the user (`dark_moods.mp4`, 14 s).

### Fixes after the first play (2026-09-06)

- **No waves in `SHOOTY_LEVEL=world`** — it had `gloom_sites: &[]` (a
  deliberate sandbox), so no vents, so no enemies. Now `world.rs` has five
  `GLOOM_SITES`, one per themed district. **Waking is by approach**
  (`gloom::WAKE_RADIUS`, 70 u) not by wave number: a source only starts brooding
  once a hero gets near it, so a district stays quiet until you walk in.
  `enemy::run_wave` now vents from the brooding source **nearest the party**
  (was: highest index), so the mob turns up where you are and you clear the map
  a district at a time. This is the M2 core, minimal version.
- **`grade.rs`** now also lifts with `Chorus::cured / 5` (districts cheered up),
  not only stage coverage — otherwise `world` (no stage) would sit at full gloom
  forever. Still global; per-district is M3.
- **Car turned too slowly at low speed** to point its way out of a jam. `grip()`
  in `drive.rs` replaces the old `speed/TOP` clamp: near a standstill the nose
  swings at ~0.85 of `TURN_RATE` (now 2.7), falling back toward speed-tracking by
  ~35 % of top speed so a car at pace still can't snap-turn. Feel-test pending.
- **Building is still unusable in `world`** (`speaker_sites: &[]`, so
  `nearest_pitch` finds nothing). A district map wants a build-at-feet fallback
  in `place_structures`, not ten hand-placed pitches — folded into M2.

### Second-play pass (2026-09-06)

- **Dark clones now read as the citizen.** `dark_skin` base colour went
  `0.14 → 0.24` (cool): dark enough to still be a shadow, light enough that the
  garment values come through — a dark-disco clone (pale suit → grey) vs a
  dark-hip-hop one (red puffer → maroon). Per-clone height jitter (0.9–1.12×) so
  a one-genre crowd isn't stamped.
- **Minimap shows reclaimed districts** — a soft amber wash around the `home` of
  every gloom source that's been cheered up (`minimap.rs`, before the
  secured-zone pass).
- **More enemies** — `wave_shape` non-capture `(14+9n → 18+10n)`, cap `105 → 120`.
  Cures still land (verified: `disco` cured at t=32 s in a town crank capture),
  but wave pacing vs the cure-a-vent-to-zero rule is worth a proper balance pass.
- **`M` on the game-over / victory screen → character select** (`hud::restart`).
  `select::clear_previous_run` tears the finished run's `RunEntity`s down on
  `OnEnter(Select)` so the lineup opens clean.

### Open / next

- **Face decal is `despairing.png` at body scale** — reads as a dark scribble
  more than a face up close. A cleaner hollow-eyes texture, or the genre atlas's
  own painted face darkened in place, would both be better.
- **Heads still read as cubes**, not heads. Using the actual `head` mesh from
  `base.glb` (a GLB primitive load) instead of a `Cuboid` is the fix.
- **`die` clip unused** — humanoid enemies still just despawn + burst. A short
  `Dying` state playing `die` before despawn would sell the cure.
- **Net client** spawns ghosts with `genre = id % 5` (the wire doesn't carry
  it). Fine for the prototype; a real fix adds a field to `EnemyWire`.
- **M2 — district reclamation.** *Started* (see Fixes above): per-district
  gloom sources, approach-waking, nearest-vent. Left: one **stem per district**
  returned on saving it (currently still one stem per source, five total);
  build-at-feet so the placeable speaker works in `world`; a per-district
  "saved" state and something hanging off "all districts saved".
  `world::district_at` names the district a point is in.
- **M3 — spatial lighting.** `grade.rs` fades the whole world off one global
  signal; make it per-district so an unsaved district sits dark and fogged next
  to a bright saved one, with a visible border.
- **Proposed for M2:** one music stem per district (≈5 districts ≈ 5 stems),
  returned when the district is saved — keeps the "clear area → music back"
  payoff at district granularity.
