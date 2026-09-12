# Shooty — Art Direction & Asset Pipeline

Status: **in-engine.** Local **two-player co-op** — blocky guitarist + drummer
(animated, skinned), one party-framing camera; face-cube enemies + the humanoid
goon; procedural music-gear pickups (amp / boombox / synth); an HDR + bloom
render pass; a lived-in Kenney low-poly **town** (roads, pavements, plaza,
bandstand, buildings) the swarm navigates; the **level loop** (discrete waves →
between-wave build lull → plant **loudspeakers** that secure + link zones →
raise the **stage** to win) with placeholder synth audio; a corner **minimap**
and a **gloom→daylight** grade that lifts as the band reclaims ground. Co-op
rework M1–M5 all landed — what's left is a balance pass (see *Next steps*).

## The look

**Everything is boxes.** Characters, environment, props, machines, VFX — one
vocabulary, no exceptions. Chunky rectangular volumes, flat colour, detail
*painted* rather than modelled, minimal faces, soft shadows, a bright toy
palette. Near-voxel without being voxel-gritty.

The production target for characters is the **Synty "Simple People" look**
(`assets/refs/characters.jpg`, `characters - repeat.jpg`): box head, slab hair,
block limbs, two dark bars and a line for a face. **BAM Squad** (VR) is the
reference for proportion and readability in motion; **Job Simulator** (VR) for
the "everything in the world is a friendly box" rule that keeps props and set
dressing in the same vocabulary as the people; **Wobbly Life**
(`assets/refs/machines.jpg`) for the environment — clean smooth low-poly,
candy palette, rounded chunky machinery.

Synty's packs are **paid**, and a full cast would be a real spend, so the
pipeline reproduces the style from CC0 parts rather than buying it (see
*Characters* below). The crisper MagicaVoxel hero/goon renders (`hero.jpg`,
`goon.jpg`) are *mood/aspiration* — not the game-ready fidelity.

**Terminology.** "Ultra-casual" / "hyper-casual" is a **market category**
(instant-play mobile, one mechanic), not an art style. Its associated look —
flat colour, primitive shapes, high contrast, no texture detail — does overlap
ours, so it is fair shorthand for the *finish*; it just says nothing about the
boxiness, which is the actual rule. For briefs and asset searches use **"chunky
low-poly"**, **"blocky / box-modular characters"**, or Synty's own range name,
**"Simple"**.

### The line that drifted (2026-09-04)

Sessions 13–15 built a cast on the **Quaternius "Ultimate Modular Men"** meshes:
one 62-bone skeleton, eleven bodies, thirteen genre skins by per-material
recolour (`src/game/skins.rs`). It is good work and it is **kept** — the whole
line is on the `art/quaternius-skeletal` branch and every asset stays under
`assets/models/quaternius/` — but it is **not the house style**. Those meshes
are realistically proportioned smooth-shaded humans; held against
`characters.jpg` the gap is obvious, and at the game's own camera distance they
read as thin indistinct smudges where a box person reads as a person.

The comparison lives in `screenshots/compare/` (`SIDE_BY_SIDE.png` and
`SIDE_BY_SIDE_GAME.png`), regenerated with:

```bash
cargo run --bin lab -- shot screenshots/compare cmp
```

One honest caveat on those sheets: the boxy cast is spawned at scale 1.0 and
the Quaternius cast at its own shipped scales, so part of the size difference
is calibration rather than style. The *silhouette* difference is not.

Reference set in `assets/refs/` (from the user's Pinterest
["shooty" board](https://ie.pinterest.com/brickley2438/shooty/)):

| Layer | Reference | Rule |
| --- | --- | --- |
| **Characters** (hero + goons) | `hero.jpg` `goon.jpg` `characters.jpg` `characters - repeat.jpg` | ONE humanoid mesh, differentiated only by **texture skin** + scale + prop. Painterly-flat UV detail (suit, shades). |
| **Simple enemies** | `box enemies.jpg` | One textured **cube** (or near-cube), face painted on. **Loud, saturated**, one hue each. Separate system from the humanoid goons. |
| **Pickups** | `synth pickup.jpg` (music theme: demo tapes, synths, amps) | Real object as a small model. Neutral body, **emissive** accents on screens/LEDs/knobs — "same material as the world, but lit up." |
| **Environment** | *Wobbly Life* (`machines.jpg` = future machinery) | Clean smooth low-poly, bright toy-like palette, soft shadows. NOT voxel. Currently a grey box — out of scope until characters land. |

### Palette (sampled from refs)

- **Hero** — stark, no accent colour. Black suit + black hair block `#0A0A0C`,
  cream skin `#E8C9A0`, black wraparound shades. Always reads as "you".
- **Goon** — near-black navy suit `#1A1F2E`, dark-brown hair `#5A3D30`, same
  cream skin, black shades. Bulkier, slightly hunched. One warm accent only:
  cigarette (white stick / `#E8622A` ember / grey smoke voxels).
- **Simple enemies** — one saturated hue each: toxic purple + green drip, black +
  glowing green eyes, bone white, acid yellow, grey + red. A pokéball as the
  "generic" one.
- **Pickups** — royal blue body `#2456C8`, glowing cyan screen `#5CE1E6`,
  red/teal buttons, glowing blue slider.

## Pipeline

### Characters — Kenney "Blocky Characters" base + custom skins

**Base mesh** (`assets/models/blocky/base.glb`, CC0, from
[kenney.nl](https://kenney.nl/assets/blocky-characters), see
`assets/models/blocky/LICENSE-kenney.txt`):

- 6 parts (`leg-left`, `leg-right`, `torso`, `arm-left`, `arm-right`, `head`),
  24 verts each — literally boxes. Node-transform rig, **no skinning**, so it
  animates the same way the game already animates things.
- **27 baked animations**: `idle` `walk` `sprint` `die` `pick-up`
  `holding-both-shoot` `holding-right-shoot` `attack-melee-*` … (full list:
  indices 0–26 in the GLB).
- All 18 Kenney skins share this exact mesh + UV layout → a "skin" is just a
  1024² PNG. Kenney's 18 are in the pack for instant grunt variety; we paint our
  own for hero/goon.

**Skin system** (to build): `Skin { palette | texture, prop, scale }` →
`spawn_character(skin)`. Examples:
- `hero` — black-suit + shades skin, `scale 1.0`
- `goon` — navy-suit skin, `scale ~1.15`, cigarette prop, `sprint` gait
- `grunt_*` — recoloured skins per wave

**Skin art**: painted outright by `tools/gen_blocky_skins.py`, from a
per-character spec (skin, hair + hair style, top, shirt, sleeve length, legs,
boots, accent, shades, beard). Earlier skins *recoloured* a stock Kenney atlas
by colour-distance (`tools/gen_skins.py`); that can only ever wear the clothes
the source was painted in — no mohawk, no stetson, no twin tails — so the cast
is now authored rather than found. Free.

The atlas rectangles the painter targets were **measured off the mesh**, not
guessed: `base.glb`'s TEXCOORD_0 per cube face, taken mod 1024 (the Kenney UVs
run outside [0,1] and need a **Repeat** sampler — see `boxy_atlas` in
`src/bin/lab.rs`; the default clamp sampler squashes every part onto the atlas
edge and the character comes out dipped in one colour).

    head    128² faces: left(0,128) front(128,128) right(256,128) back(384,128)
                        top(128,0)  bottom(128,256)
    torso   band y784-928: side(0) front(96,w128) side(224) back(320,w128)
                        top(96,688,128x96)  bottom(96,928)
    arm     island x480 / x768: top+bottom caps (64²) at y534, band y598-768
    leg     island x480 / x768: top+bottom caps (64²) at y800, band y864-1024

Image top = model top on every part, so a sleeve is the top of the arm band and
a boot the bottom of the leg band.

**The cast** is `skins::BOXY` — five rockers (punk / metal / grunge / glam /
rockabilly, each also a band role so the five read as one group) and five genre
crowds (disco / techno / hip-hop / country / J-Pop). Heroes get the warm,
high-contrast reads; crowds get the loud, cool, high-key ones, so a wave reads
as one hue before you can pick out a person in it.

### Simple enemies — procedural cube + face texture

Cube mesh + a painted face texture per type. Generate faces with Grok pixel-art
(2¢) or hand-paint. Keeps the "monster tier" distinct from the humanoid "goon
tier". Replaces the current Tripo `rusher`/`sponge`/`flyer` blobs.

### Pickups — one small model each

Low-poly model + emissive material on the lit parts. A cheap Tripo model is
acceptable here (one-offs, not a skin family). First pickup = demo-tape or synth
(music theme → fire upgrade).

### Environment — hand-built / CC0 kit

Wobbly-Life-clean arena: flat bright ground, simple block walls, a few chunky
props. Kenney / KayKit / Quaternius CC0 environment kits when we get to it.

## Scale — the metric standard (2026-09-04)

Sizes are decided once, in `src/game/scale.rs`, and stated in **metres**. See
the README's *Scale* section for the numbers and for the three measurement
bugs it exposed (city-kit heights a unit too large, `BOXY_LIFT` floating the
whole boxy cast, cars fitted by length instead of height).

Two rules that come out of it and are worth keeping:

- **A hero is the tallest thing on the street.** Not literally — buildings and
  trees tower — but among the things you move past at speed: cars, bins,
  benches, hedges, and every enemy short of a goon. If a hero can be hidden
  behind a parked car at the top-down camera, the car is too big.
- **A wave is dark moods, not monsters and not the citizens themselves.**
  (2026-09-06 — replaces the old "bad vibes are creatures" rule and the
  saturated face-cubes.) Every enemy is the boxy `base.glb` rig wearing the
  venting citizen's own genre atlas, multiplied down to a drained near-black
  with a genre-hued glow where its face should be — recognisably that person,
  gone wrong. Tiers in `enemy.rs`: **Mood** (a walking clone, a head shorter
  than the 1.8 m citizen — a diminished version of them), **Head** (just the
  head, detached, drifting), **Heckler** (hangs back at ~13 m and throws a
  dark bolt), **Sink** (oversized — the only tier allowed to loom over a hero).
  They read as a threat by arriving in numbers and, now, by shooting back.

## The shot is a sound wave (2026-09-04)

`ShotLook::Wave` replaces the graphic-EQ bar as the default. The reasoning is
worth writing down because it generalises: an EQ bar or a quaver is a *picture
of* music — a symbol you have to already know — whereas a wave is the shape
sound actually has, so it needs no decoding. It is also the only one of the
candidates that can carry **character**: waveform (timbre), cycles (pitch),
swing (loudness) and colour all come from `skins::sound(id)`.

Two build notes:

- It is drawn as boxes **spanning between samples**, not boxes sitting *on*
  samples. Sitting on samples gives a dotted line with holes wherever the wave
  turns hard — a square wave came out as scattered blocks. Spanning gives the
  vertical risers you would draw by hand.
- Sixteen phases are baked as meshes at first fire and swapped per frame. The
  oscillation has to be free: forty shots can be in the air at once.

## Abandoned

- **Tripo image-to-GLB** (current player + 3 enemies): no shared topology, can't
  skin-swap, comes out blobby — the root cause of "the rusher looks like a
  virus". **Keep the files** in `assets/models/*.glb` (esp. `player.glb`, the
  rocker — user may reuse it); just not wired into the game.
- **MagicaVoxel sculpted `.vox`** (`bevy_vox_scene`): gorgeous but every model is
  bespoke — kills "one mesh, many skins", and no one is authoring `.vox`. Keep in
  reserve for a title-screen hero render only.

## Story / world (evolving — user direction 2026-09-03)

- The enemies are the **dark moods** of a town starved of music — and, since
  2026-09-06, they *look* it: each one is a drained, glowing-faced clone of the
  citizen it spilled out of (`enemy.rs`, four tiers), not an abstract cube. You
  are never fighting the townsfolk themselves — those five stand off at a
  distance leaking the crowd — so clearing a wave visibly cheers its source up.
  **Music is the weapon that dispels them** (the drummer's beats, the stage's
  pacify field, and clearing a source returns its stem).
- A **level** is the gradual reclamation of a town: clear enough ground, one
  zone at a time, by erecting **loudspeakers** (old-fashioned klaxon/horn PA
  speakers on a pole). Speakers **link up**; the level is won when the band
  raises a full **stage** — drum kit, keys, guitar amps, a central mic — on the
  secured ground. The **win sequence** puts the band on that stage playing (can
  come later).
- As zones are reclaimed the **look shifts with them**: gloom → warm, well-lit;
  desaturated → the toy-bright Wobbly-Life palette. Drive it off the same
  secured-fraction signal that already fades the music bed
  (`build::SecuredZones::music_level`).

## Future / not in current scope (roadmap beyond M4)

Sequenced roughly; each is ~1 conversation, start fresh after (same cadence as
M1–M4). See the memory scope note for the running detail.

- **M4 — the level loop.** ✅ Done (2026-09-03). Amp turret → **loudspeaker**
  (the klaxon GLB), permanent, secures a radius-19 zone; `SpeakerNet` links
  speakers whose zones overlap; 3 linked → **stage** unlocks; finishing it →
  `GameState::Victory` + win screen. Discrete `Wave` state machine
  (Prep/Spawning/Clearing) with a between-wave build lull. Win *sequence* (band
  onstage) still deferred.
- **M5 — coverage feedback.** ✅ Done (2026-09-03). Bottom-right **minimap**
  (`minimap.rs` — a CPU `Image` redrawn each frame: town + road grid, secured-zone
  glow discs, hero / swarm / speaker / stage blips) and the **gloom→daylight**
  world grade (`grade.rs` — `WorldGrade` eases toward `music_level`, lerps sky /
  ambient / fog / key sun between a cold overcast and warm daylight). No second
  camera needed — the minimap is a UI `ImageNode`.
- **M6 — LAN multiplayer.** Real networked co-op, **LAN only, no matchmaking**
  (was deferred as a separate project; now queued). The `Intent`-component
  input design is already the right shape — serialise `Intent`, replicate
  entities (`bevy_replicon` / `lightyear` / raw `renet`). Local shared-screen
  co-op stays as a mode.
- Wobbly-Life world expansion — cartoony low-poly machinery / vehicles ref in
  `assets/refs/` (construction trucks, cranes, diggers; candy colours, rounded
  chunky forms).

## Build log

- **2026-09-04**: Music gear, carry poses, and what a shot looks like.
  - **`src/game/props.rs`** — the instruments (flying V, bass, snare, synth,
    mic stand) as chunky flat-colour primitives, plus the **carry poses** that
    put one on a body: `Slung` (across the chest at 45°), `Aim` (right hand),
    `Back` (stowed flat). A pose is a bone name plus a local transform, keyed
    on the [`Rig`] — Quaternius `Chest`/`Wrist.R` vs blocky `torso`/`arm-right`.
  - **The guitar no longer lives on the wrist.** It used to be parented to
    `Wrist.R` permanently with `aim_guns` twisting it at the cursor every
    frame, so it floated off the hand at an odd angle through every walk, idle
    and roll. `player::carry_guns` now hangs it across the chest at rest and
    moves it to the hand only while the guitarist is firing (plus a 0.45 s
    linger so tapping fire doesn't make it flicker). `aim_guns` skips anything
    not in `Carry::Aim`.
  - **Mount numbers were measured, not guessed.** Mounting a prop at each
    bone's origin unrotated and rendering it showed that both rigs' mount bones
    share the model's own axes (+Z forward, +Y up), and that the blocky `torso`
    node's origin is at the **hip** — its box runs y 0.3–1.2 above it. The
    first pass guessed a chest offset and put the guitar at the character's
    knee.
  - **`weapon::ShotLook`** — the projectile is selectable: `Bar` (one bar of a
    graphic EQ, now the default), `Chord` (a spectrogram slice), `Note` (a
    quaver), `Pulse` (a beat ring), `Pick` (the original plectrum). Shots now
    face down their own travel instead of tumbling; only `Pick` still spins.
    The plectrum read as a slice of pizza precisely *because* it tumbled.
  - Lab: `B` cycles the carried prop, `M` cycles the carry pose, and
    `cargo run --bin lab -- shot <dir> gear` shoots every prop in every pose on
    one body of each rig — back poses from the back view, hand poses over the
    shoot clip — plus every projectile candidate at the true in-play framing.
    `capture` takes a 4th arg naming a shot look, so the call can be made from
    footage.
  - Bug the sheet exposed: the lab's nudge mode was overwriting *every* mounted
    prop's transform with the nudge defaults whether or not nudging was on, so
    carry poses silently came out shrunk and in the wrong place.
  - **Corrections after review**: worn gear now sits **flush** — every prop is
    modelled face-up (neck down +Z, face normal +Y), so a worn pose needs an
    X-quarter-turn to point the face away from the body *before* the roll that
    sets the hanging angle. Rolling alone stood the instrument on edge against
    the chest. Standoff is the torso half-depth plus the prop's own
    half-thickness, so the back of the guitar rests on the body rather than
    half inside it. The `Aim` pose gained the two quarter-turns that put the
    neck down the arm (X sends the neck from +Z to -Y, the direction a limb
    runs in both rigs' bone space; Y rolls it upright about its own neck).
  - **Worn gear does not read from the top-down camera.** At the true in-play
    framing a slung guitar is a visible red mark on the chest; one stowed on
    the back is almost entirely hidden by the body. Back-carry is for close-ups,
    menus and the win sequence, not for gameplay silhouette.
  - Still open: the boxy rig's `Aim` pose is only roughly placed (it is not
    wired into the game yet, and `aim_guns` would override its rotation there);
    the quaver is illegible at in-play size.

- **2026-09-04**: Back to boxes — the boxy cast, and the Quaternius line parked.
  - The art direction had drifted: sessions 13–15 built the cast on Quaternius
    skeletal meshes, which are realistically proportioned smooth-shaded humans,
    not the Synty-Simple box people the brief asks for. **Nothing deleted** —
    the line is on `art/quaternius-skeletal`, the assets stay under
    `assets/models/quaternius/`, and `skins::SKINS` still drives the game.
  - **`tools/gen_blocky_skins.py`** — paints the Kenney blocky atlas outright
    from a per-character spec (hair style, open jacket, sleeve length, boots,
    collar accent, shades, beard). Ten characters, ten 1024² PNGs, no spend.
    Atlas rects measured off `base.glb`'s UVs rather than eyeballed.
  - **`skins::BOXY`** — the ten-strong cast as data, next to `SKINS`, so the
    lab and the game read one table. `BOXY_BASE` / `BOXY_LIFT` carry the mesh
    and its hip-origin lift.
  - **Asset lab** grew a `Subject::Boxy` and `cargo run --bin lab -- shot <dir>
    cmp`, which shoots the boxy cast and the same-genre Quaternius skins for a
    frame-for-frame comparison (`screenshots/compare/`).
  - Two bugs the comparison exposed: the lab opened every Kenney model
    mid-kick (it looked for `Idle`, Kenney names it `idle`), and the first pass
    of the open-jacket panel read as a giant arrow on the chest (a constant
    strip reads as a tie; one with its top corners cut reads as an arrow — it
    has to *widen* from the throat).
  - Still open: the boxy cast is view-only in the lab. Wiring it into
    `player.rs` / `enemy.rs` — five playable heroes and five enemy waves — is
    the next job, and needs the scale calibration doing properly.

- **2026-09-04**: **Quaternius base-mesh milestone (WIP).** Moved the hero,
  drummer and goon off Kenney `blocky/base.glb` (27 node-transform clips, no
  skeleton) onto the **Quaternius "Ultimate Modular Men"** pack (CC0, vendored
  to `assets/models/quaternius/`, 11 characters converted .gltf→.glb) — one
  shared **62-bone skeleton** with segmented limbs + 24 baked **skeletal**
  clips.
  - `player.rs`: guitarist wears `punk.glb` (reads as an aging rocker straight
    off the shelf — red mohawk, black), drummer `casual.glb`. Feet already at
    y = 0 (no hip-origin lift), `PLAYER_MODEL_SCALE 1.15`. Gait → Quaternius
    clips: `Idle_Gun`/`Idle` · `Walk` · `Run` · `Idle_Gun_Shoot` · `Sword_Slash`
    · `Death`. Guitar-gun clips into the `Wrist.R` bone (`PlayerHand`).
  - **Reskin is now per-material, in-engine** (Quaternius meshes are flat
    material colours, no texture atlas / no face UV): `hero_tint()` /
    `enemy::gloom_tint()` remap `StandardMaterial.base_color` by
    `GltfMaterialName`, cloning each tinted material once. `gen_skins.py` (atlas
    remap) is now dead.
  - **Face-plane system** (user picked "build it"): `tools/gen_face_planes.py`
    bakes the locked **synty rect-eye** face (painter lifted from
    `gen_people.py`) to `assets/models/quaternius/faces/{hero,drummer,gloom}.png`
    — **RGBA, features only, transparent field**, so only the brows/eyes/nose/
    mouth land on the character's own head skin (no card, and the gloom face
    can't keep a healthy skin tone against a grey body). The engine parents a
    `Rectangle` quad — `player::face_in_head()` + `face_material()`, shared by
    `player.rs`, `enemy.rs` and the lab — to the `Head` bone (heroes) / model
    root at head height (goons, to dodge the spawn/despawn race). Hair stays
    **stock Quaternius** (recoloured), no block system. The `suit` goon's stock
    **pistol mesh is hidden** (`on_goon_ready`, by node name).
  - `enemy.rs`: goon = `suit.glb` greyed by `gloom_tint` ("gloom grey" — a
    drained office worker), `GOON_SCALE 1.28`, forward hunch kept, `Walk` loop.
    Dropped the Kenney `torso`/`arm-*` chest-rescale (silhouette difference now
    comes from suit-vs-punk).
  - **Left / open**: guitar-gun fit wants a tuning pass (fine from behind,
    loose top-down); hair-block attach system (memory `shooty-people-artdir`)
    not built — Quaternius hair may be enough; face-cubes now read oversized
    next to person-scale heroes; `Death` clip unused for goons (still just
    despawn+`Explosion`). `blocky/` kept until sign-off.
  - Proof: `screenshots/people_cam/shooty_quaternius_wip.mp4`.

- **2026-09-04**: **People art direction locked + parallel engine pass.**
  - `tools/gen_people.py` (non-engine) — procedural rect-eye faces on the
    head UV, chunky fake-iso bust render, 1–4-block hair/hats, limb comparison.
    Contact sheets in `screenshots/people/`. Decisions (memory
    `shooty-people-artdir`): **synty rect-eye** face family, **gloom-grey**
    enemy townsfolk, **Quaternius CC0** segmented-limb base mesh (replaces
    `blocky/base.glb` — own milestone), hair = attached blocks not paint.
  - **Death pop** (`game/vfx.rs`, `VfxPlugin`) — `Explosion` message →
    kind-tinted cube-shard burst (ballistic + tumbling, ground-bounce), white
    core flash + expanding emissive ground ring (`Grow`) + a point-light punch.
    Shared meshes + per-kind materials, zero alloc per burst. `combat::check_death`
    now fires `Explosion` instead of the old 0.1 s spark sphere.
  - **Camera modes** (`game::CameraMode`, **V** to cycle) — `Follow` (unchanged
    co-op framing) / `Shoulder` (street-level OTS down the guitarist's aim) /
    `FirstPerson`. `follow_camera` → `drive_camera` dispatcher; `bin/capture.rs`
    `cycle_cam` scripts a 6 s rotation for the proof video.
  - Proof: `screenshots/people_cam/shooty_cam_explosion.mp4`.

- **2026-09-03**: **M5 — coverage feedback** (new `game/minimap.rs`,
  `game/grade.rs`; `lib.rs` `KeySun` marker; `game/mod.rs` wiring).
  - **Minimap.** `MinimapPlugin` builds one 192×138 `Rgba8UnormSrgb` `Image`
    (`MinimapTex`, reused across runs), shown bottom-right in a framed
    `ImageNode` (`ImageSampler::nearest`). `draw_minimap` rewrites the pixel
    buffer every frame: grass / road base via `Level::on_road`, secured-zone glow
    discs scaled + brightened by `SecuredZones::music_level`, then blips for the
    swarm (red), build sites (pulsing amber), loudspeakers (cyan), the stage
    (magenta/white) and the two heroes (hero colours, dimmed when downed).
  - **World grade.** `GradePlugin` — `WorldGrade(f32)` eases toward
    `music_level() / 0.78` (so a live stage → full daylight); `lift_the_gloom`
    smoothersteps it and lerps `ClearColor`, `GlobalAmbientLight`, the camera
    `DistanceFog` (colour + linear falloff) and the `KeySun` `DirectionalLight`
    (illuminance + colour) between a `GLOOM` (cold overcast, fog closing in,
    ~4200 lux) and a `LIT` endpoint (the scene's original warm 20k-lux day).
    `KeySun` marker added to the warm sun in `lib::spawn_key_light` (both bins).
  - Proof video → `screenshots/m5/shooty_m5_coverage.mp4`.
- **2026-09-03**: **M4 — the level loop** (`build.rs` reworked; `enemy.rs`,
  `hud.rs`, `mod.rs`, `player.rs`, `bin/capture.rs`; new `tools/encode_video.py`).
  - **Loudspeaker.** `StructureKind::Amp` → `Speaker`, permanent, carries the
    klaxon GLB (`assets/models/klaxon.glb` — Grok ref → Tripo3D, ~32¢).
    `on_speaker_ready` observer drives `emissive` off the baked base-colour
    atlas so the cyan horn mouths bloom without a separate emissive map.
    On completion it registers a `SecuredZones::Zone { center, radius: 19 }`
    (was stage-only) and lays suppressing `speaker_fire` bolts at the zone edge.
  - **Linking.** `SpeakerNet` + `update_speaker_net`: union-find over speakers
    whose centres are within `SPEAKER_LINK_RANGE` (zones overlap), tracks the
    largest connected cluster, and rebuilds glowing ground **beam** entities
    when the link set changes. `place_structures` gates the stage on
    `net.largest >= SPEAKERS_TO_LINK` (3).
  - **Victory.** `GameState::Victory` (new). `channel_sites` sets it when a
    stage finishes; `hud::spawn_victory` shows the win screen, `R` restarts
    (shared `restart` now runs in GameOver **or** Victory). Stage got a back
    line of amp cabs + a mic stand so it reads as the band kit. Win *sequence*
    (band playing) deferred.
  - **Discrete waves.** `enemy::Spawner` (continuous trickle) → `Wave` state
    machine: `Prep` (calm build lull, HUD countdown) → `Spawning` (a
    `wave_shape` budget poured in as bursts) → `Clearing` (wait for the field
    to thin) → next `Prep`, each wave bigger. Capture-only lull trickle +
    `capture::auto_restart` (loops ~3.5 s after Victory) keep the proof video
    moving.
  - `bot_input` rewritten for M4: both heroes rush/hold any active site; the
    guitarist walks a fixed 3-point loudspeaker triangle, the drummer raises
    the stage at their centroid once linked.
  - Proof: `screenshots/m4/shooty_m4_levelloop.mp4`.
  - **Known**: `SecuredZones` is now `Vec<Zone>` (per-zone radius). Balance is
    first-guess. `RunClock` isn't reset on the capture auto-restart (cosmetic).

- **2026-09-03**: **M3 — build phase + audio** (new `build.rs`, `audio.rs`;
  `enemy.rs`, `combat.rs`, `hud.rs`, `player.rs`).
  - **Scrap economy.** `EnemyDied` now carries the `EnemyKind`; `build::drop_scrap`
    mints a homing `ScrapMote` per kill (value 1/2/3/5 by tier) that arcs up,
    falls, then chases the nearest hero into a shared `Scrap` pool. HUD counter.
  - **Structures.** `Intent.build` (`Q` guitarist / `;`+pad-North drummer) drops
    a `BuildSite` at the hero's feet and pays for it; `channel_sites` fills its
    progress while heroes stand within `CHANNEL_REACH` (faster with two), decays
    unattended. **Amp turret** (guitarist, 20 scrap, ~2.4 s) — a dark cab with
    an emissive face that auto-fires cyan bolts at the nearest enemy for 18 s
    then crumbles (breathing room). **Stage** (drummer, 70 scrap, ~6.5 s) — a
    two-tier timber deck with coloured stage-light spheres; permanent.
  - **Secured ground / frontier.** A finished stage pushes its centre into
    `SecuredZones`. `enemy::spawn_enemies` `push_out`s the spawn ring past every
    zone edge, so the swarm recedes to unclaimed streets. `build::pacify_zone`
    tags any enemy that wanders into a zone `Pacified` (+ a 3.5 s `Lifetime`);
    `enemy::move_enemies` then drifts it back out at a crawl and it deals no
    contact damage (`combat` skips `Pacified`). An unfinished site triples the
    spawn interval nearby — the "player-triggered lull". Visual: a faint
    additive ground disc + a bright emissive perimeter ring (NOT an enclosing
    dome — the top-down camera sits inside the radius; the sphere version
    turned the whole screen to dusk).
  - **Audio** (`audio.rs`). Gameplay fires `AudioCue` messages; `play_cues`
    spawns `PlaybackSettings::DESPAWN` one-shots, `music_bed` rides an
    `AudioSink` volume toward `SecuredZones::music_level`. Stems are numpy-synth
    placeholders (`tools/gen_audio.py` → `assets/audio/*.ogg`, Vorbis via
    `soundfile`): stick count-in, distorted power-chord stab (Encore / beat
    slam / stage complete), soft beat click (every drummer beat), scrap blip,
    build clunk, and a seamless 124 BPM bass+drums loop. `ffmpeg` isn't on the
    box; `soundfile` was added to `.venv`.
  - Capture: `spawn_enemies` `AutoPlay` branch also skips the lull and runs a
    denser horde; `bot_input` places + channels structures and roams toward
    unsecured ground. Verified `screenshots/m3/shooty_m3_build.mp4` — scrap
    motes, amp turrets holding a line, a stage deck, the pacify field, and the
    frontier arc (calm inside, swarm outside) all read. **Balance is now
    genuinely overdue** — kill/scrap counters run to the hundreds; stages never
    expire so a long game fully pacifies the map.

- **2026-09-03**: **M2 — local co-op + the drummer** (`player.rs`, new
  `drummer.rs`, `combat.rs`, `hud.rs`, `mod.rs`).
  - **Two heroes.** `Hero::{Guitarist, Drummer}` on two `Player` entities,
    spawned either side of the crossroads. Input funnels through a per-player
    `Intent` component written by `p1_input` (kbd+mouse), `p2_input` (gamepad,
    with an always-live Arrows + `/,.` keyboard fallback so one person can drive
    both), and `bot_input` (capture). Everything downstream — `movement`,
    `weapon::fire`, `drummer::*`, `drive_animation` — reads `Intent`/`Aim` only;
    the old scattered `autoplay`/mouse-button checks are gone.
  - **Drummer skin** = `tools/gen_skins.py` `drummer` entry: burnt-orange
    hair/beard, warm-grey sleeveless top, faded-red shirt, olive trousers —
    the warm/light opposite of the stark-black guitarist so the two never blur
    at the top-down camera. Amber ground ring vs the guitarist's cyan.
  - **Drummer kit** (`drummer.rs`), distinct from the guitarist's aimed picks:
    a 124 BPM `Beat` fires a `Shockwave` ring off the drummer every beat (fat
    one on the downbeat) — passive area denial; `Intent::attack` swings the
    sticks (`attack-melee-right` clip + a `CircularSector` wedge VFX, arc
    hit-check with knockback, tight cooldown); `Intent::special` slams a
    full-size shockwave on a longer cooldown. All state on one `DrumKit`
    component so `drive_animation` can read `is_swinging()` for the melee gait.
  - **One shared framing camera.** `follow_camera` now centres on the party
    midpoint and eases the camera *outward* along `CAMERA_OFFSET` as the two
    spread apart (`CAMERA_SPREAD_ZOOM` / `_MAX`), margin-clamped to the arena.
  - **Downed + revive** (`combat.rs`): a 0-HP hero gets `Downed` + `Reviving`
    (plays `die`, swarm retargets via `nearest_player`, can't act); a standing
    team-mate within `REVIVE_REACH` for `REVIVE_SECS` restores them to
    `REVIVE_HEALTH`. The run only ends when *every* hero is down.
  - **HUD**: two `HealthFill(Hero)` bars (guitarist left/cyan, drummer
    right/amber) + a `HeroLabel` that shows "DOWN (revive %)".
  - Enemies + pickups retargeted to nearest-of-N players / party centroid.
    `spawn_enemies` has an `AutoPlay` branch that runs a much denser horde
    (tighter ring, ~2× batch, cap 190) so the capture still reads as a swarm —
    two heroes + the beat AoE out-clear the normal spawn rate badly (the
    **balance pass is now genuinely overdue**; kill counter runs to the
    hundreds in the proof clip).
  - Verified: `screenshots/m2/shooty_m2_coop.mp4`.

- **2026-09-03**: Town enrichment pass — "lived-in" low-poly town (then `game/env.rs`, now `game/level/town.rs`).
  - **Pavements + crossings**: pale-concrete slabs (mesh primitives) down both
    kerbs of every road, zebra crossings (emissive white bars) on all four arms
    of every intersection. Blocks now read as blocks; buildings sit on them.
  - **Block-based buildings**: the old fully-random scatter is replaced by a
    4×4 grid walk (`place_commercial` / `place_suburban`). The four central
    blocks are Kenney *City Kit (Commercial)* towers (taller, existing tint
    palette); the 12 ring blocks are Kenney *City Kit (Suburban)* houses
    (`assets/models/suburban/`, CC0) on a colormap material tinted
    cream/sky/sage/terracotta, each with a garden tree and an optional
    `fence-1x4`. Two central blocks are carved out for the plaza and the park.
  - **Central plaza** (off the spawn crossroads at ~+29,+20): octagonal paved
    apron, a ring of 8 planters (Kenney `planter.glb` + nature bush), a
    primitive stone **fountain** with an emissive water disc, and four benches.
  - **Bandstand** (~+15,+25.5, foreshadows M3 "build a stage"): a low octagonal
    timber stage — two step tiers, a plank deck, six short posts each capped
    with a coloured emissive **stage light** (magenta / cyan / amber for the
    bloom pass). Thin `Obstacle` (r 3.6). No roof, so it still reads from the
    top-down camera.
  - **Park** (~-30,-20): a translucent-blue primitive **pond** with a stone lip
    and a keep-out `Obstacle`, a denser ring of trees, two benches.
  - **Street furniture** down the pavements (all non-colliding): Kenney
    *City Kit (Roads)* `light-curved` lamp posts each with an emissive bulb,
    Kenney furniture-kit benches, primitive fire hydrants, furniture-kit bins,
    and road signs. **Parked cars** along the kerbs — Kenney *Car Kit*
    (`assets/models/cars/`, CC0), ~12–15, colliding (`Obstacle` r 1.9).
  - All new kits render white on the stock glTF material here (same as the city
    kit), so a `WorldInstanceReady` observer re-points every mesh at a shared
    point-sampled `colormap.png` material (`spawn_prop` / `spawn_kit_building`).
    Each kit ships its own `Textures/colormap.png` + `LICENSE-kenney.txt`.
  - Added instances ≈ +110 GLB (≈ +200 primitive entities, all sharing a
    handful of meshes/materials). Verified in capture
    (`screenshots/env_m2_final/`, `town_m2.mp4`): no white meshes, nothing
    floating, no props on the roads or blocking the spawn, 480 frames clean.

- **2026-09-03**: Graphics pass + town map (milestone 1 of the co-op rework).
  - **Rendering**: shared `lib::camera_bundle()` / `spawn_key_light()` for the
    windowed game and the capture bin. HDR camera + `TonyMcMapface` tonemapping
    + `NATURAL` bloom so every emissive glows; ambient dropped 1800→380 with a
    warm 20k-lux key sun + dim cool sky-fill for real contrast and shadow.
    `DistanceFog` fades the far edge into the sky.
  - **Town** (`game/level/town.rs`, arena 100×72): grass, a 3×3 road grid (kerb +
    tarmac + dashed centre line, player starts on the crossroads), perimeter
    hedge, ~26 Kenney city buildings in the blocks (off-road, tinted), and
    Kenney Nature Kit trees/rocks — street trees along the kerbs + a park
    scatter. `Level::on_road(p)` gates placement. Nature GLBs bake a minty
    `leafsGreen` / orange `woodBark` at `metallic:1`, so a `WorldInstanceReady`
    observer routes each mesh to a deep-green `foliage` or brown `bark`
    material by base-colour hue (deep green keeps trees distinct from the
    grass-green rusher cubes). `assets/models/nature/` = Kenney Nature Kit, CC0.

- **2026-09-03**: Kenney city kit replaces the placeholder blocks.
  - `assets/models/city/` — Kenney "City Kit (Commercial)" 2.1, CC0
    (`LICENSE-kenney.txt`). 10 building GLBs (`building-a..h` +
    `building-skyscraper-a/b`) share one `colormap.png` palette atlas.
  - `reset_run` scatters ~17 of them (`CITY_MODELS` table = path + native
    height). Every model is a 2×2 footprint, feet at y=-1: uniform scale `s`
    → `2s` wide, lifted by `s`. `s = rand(4.2..7.4) / native_h` lands every
    building in a ~4–7 u height band, so squat models come out fat and the
    skyscrapers come out slim. `Obstacle { radius: s*1.18 }` keeps the existing
    circle-based collision/avoidance.
  - The stock glТF material renders **untextured white** here (and its baked
    mips grey out the palette anyway), so an `On<WorldInstanceReady>` observer
    swaps every building mesh to one of 5 shared `StandardMaterial`s: a
    point-sampled `colormap.png` multiplied by a saturated tint (steel blue /
    amber / cyan / bone / indigo — hues kept off the enemy palette) plus a
    faint matching `emissive` so shadowed faces keep their colour. Picked at
    random per building. The low-detail LOD models only sample the pale-grey
    band of the atlas — use the full `building-*` set.

- **2026-09-03**: Musical pickup models + Synth/Arpeggio.
  - The two pickups are now chunky low-poly music gear built from unit
    cube/cylinder parts (`pickup::{amp,boombox,synth}_parts`, `boxy`/`disc`/
    `knob` helpers), royal-blue bodies with bright emissive cyan panels and
    red/teal controls. Each leans on one silhouette cue that survives the
    top-down cam: **amp** = tall cab + glowing face (Encore), **boombox** =
    handle arch over a wide body (Dual guitar).
  - New third pickup **synth** → `RapidFire` buff ("Arpeggio"): `weapon::fire`
    drops the cooldown to 0.38× for `BUFF_SECS`. Low blue wedge with a white
    keybed along the front edge. `PickupKind::ALL` drives the timed rotation
    (Encore → Boombox → Synth) and the kill-drop roll; `expire_buffs` clears it.

- **2026-09-03**: Env blocks + navigation, goon build, guitar-aim fix.
  - **Placeholder city blocks**: `reset_run` scatters ~13 chunky boxes
    (`Obstacle { radius }`, circle-approximated) through the arena, clear of the
    centre. `resolve_obstacles` (in `game/mod.rs`) push-resolves the player and
    grounded enemies out of them; bullets despawn on contact. **Enemy
    navigation** = local tangent-steering avoidance in `move_enemies`: any block
    between an enemy and the player bends its heading around the near side
    (flyers ignore it and cruise over). The capture bot got the same avoidance
    so it doesn't grind into walls. No A* — good enough for a swarm, will show
    local-minima stalls behind big blocks if pushed.
  - **Goon is a different build from the hero**: `on_goon_ready` non-uniformly
    scales the `torso` node (`GOON_CHEST` 1.5×1.12×1.5 — barrel chest) and
    counter-scales its `arm-*` / `head` children (blocky rig parents them under
    `torso`) so only the chest stretches; arms end ~1.18× thick, head ~0.92×.
    The `walk`/`idle` clips carry no scale channels so these stick.
  - **Guitar-gun pointed backwards** after the goon commit — `aim_guns` read the
    arm's propagated `GlobalTransform` from `Update`, a frame behind the
    animation; during a fast shoot-swing that lag spun the gun ~180°. Now runs
    in `PostUpdate` after `TransformSystems::Propagate` and writes the gun's +
    its parts' `GlobalTransform`s directly off *this* frame's arm pose.
  - Goon balance trimmed to read better on camera: 78 hp, spawn share ~7%→17%.

- **2026-09-03**: Humanoid goon enemy in.
  - `EnemyKind::Goon` — the same Kenney blocky base mesh as the hero, worn in
    the `goon` skin (navy suit / brown hair / cream face), walking upright.
    Shared `GoonAssets` resource: one scene handle, one skin material, one
    `walk`-loop `AnimationGraph` (built by `prepare_goon_anim` once the GLB
    loads). Each goon = an `Enemy` parent `move_enemies` steers like any grunt,
    carrying the mesh as a child; `on_goon_ready` (per-entity observer) paints
    the skin on every body part and starts the walk loop, desynced per goon
    (random speed 0.85–1.1 + random phase) so the horde doesn't march in
    lockstep. `GOON_SCALE` 2.15 (a touch over the hero's 2.0), 0.13 rad forward
    hunch, ±6% size jitter. Stats: 95 hp / speed 5.6 / 15 touch — a slow
    bruiser. Spawn share ramps ~9% → ~22% over the first minute, gated on the
    walk graph being ready. Verified in capture.
  - Fixed a latent bug the goon exposed: `attach_guns` found `arm-left` /
    `arm-right` by name across the *whole* world, so a second blocky mesh could
    steal the hero's guitar-gun. The hero's hand nodes are now tagged
    `PlayerHand` in `on_model_ready`; gun attachment only looks at those.
  - Not done: cigarette prop, goon-specific death (they use the shared spark).

- **2026-09-03**: Weapon on the hand + musical pickups.
  - Guitar-gun reparented from a free-floating carried entity to the
    `arm-right` **node of the animated mesh** (`player::attach_guns`, found by
    `Name`). It rides the animation; `aim_guns` sets its local rotation each
    frame to `arm_global.rotation().inverse() * face_aim` so the neck tracks
    the cursor regardless of the arm pose. Shots originate from a `Muzzle`
    child's `GlobalTransform`.
  - Projectiles → **spinning guitar picks** (`Extrusion<Triangle2d>` +
    `weapon::Spin`), gold emissive, + a muzzle-flash sphere.
  - `pickup.rs`: floating pickups (timed, alternating, ~8 s; + 3.5% kill-drop
    via the `EnemyDied` message). **Dual guitar** → a second gun on `arm-left`,
    both muzzles fire. **Encore** → RMB spawns a `SoundWave` (expanding
    emissive torus, one-time damage + knockback per enemy as the wavefront
    passes). Buffs last 12 s. Bot detours to pickups and auto-fires Encore into
    crowds. Wants audio: a "1-2-3-4" count-in into a big chord.
  - Capture balance: enemy spawn ring pulled in to 11-21 u, batches 8-19,
    cap 210 so the horde stays on-screen; the two pickups together still
    over-clear — a real balance pass is owed.
- **2026-09-02**: Pipeline chosen. Kenney Blocky Characters pack vendored to
  `assets/models/blocky/`. Player swapped from Tripo `player.glb` to
  `blocky/base.glb` (`player.rs`, `PLAYER_MODEL_SCALE 1.6`, hip-origin lift
  `1.0 * scale`) — loads + textures + casts shadow, verified in capture.
- **2026-09-02**: Player animation wired (`player.rs`). `base.glb` ships 27
  node-transform clips (no skin/skeleton — the 6 body-part meshes are animated
  as nodes). `WorldInstanceReady` observer (`on_model_ready`) finds the baked
  `AnimationPlayer` in the spawned scene, builds an `AnimationGraph` from
  `idle`/`walk`/`sprint`/`holding-both-shoot`/`die`, starts it idling.
  `drive_animation` picks a gait each frame from player state
  (GameOver→die, rolling→sprint, firing→shoot, moving→walk, else idle) and
  cross-fades 140 ms via `AnimationTransitions`. Verified in capture: limbs
  cycle through the shoot clip. No prop/gun mesh yet.

- **2026-09-02**: Bigger arena + face-cube fix.
  - `ARENA` 19×12 → 52×34 half-extents (~7.5× area); `follow_camera`
    (`game/mod.rs`) trails the player at `CAMERA_OFFSET`, clamped so the walls
    sit near the frame edge. Grid-textured floor (`assets/textures/floor.png`,
    Repeat sampler + `uv_transform`) so movement through the space reads; slab
    overshoots the walls. Enemies now spawn in a ~30 u ring around the player.
  - **Face-cubes reworked**: the solid cube + separate camera-facing decal quad
    read as "two boxes". Replaced with `enemy::face_cube()` — a custom cube
    mesh, whole face texture mapped upright on every side (winding: face-space
    right = `up × normal`). One material per kind, no decal, no `Body` split;
    the cube itself turns to the player / tumbles.
- **2026-09-02**: Hero glow-up.
  - **Skin swap needs a repeating sampler.** The Kenney meshes carry UVs outside
    [0,1] (V ≈ 1.0–2.0) that land on their atlas island only with `Repeat`
    addressing; the default clamp sampler squashed every part onto the atlas
    edge (blue arms, dark head). `PendingSkin` now loads via `load_builder()`
    with a Repeat + Nearest `ImageSamplerDescriptor`.
  - `gen_skins.py` rewritten: classify each texel to its nearest source anchor
    in `texture-p` and remap by role (hair/jacket/shirt/legs/boots), keeping
    per-texel shade. Hero = **platinum hair** (reads from directly above),
    black leather jacket, denim jeans, black boots. Only colour on the body.
  - **Red flying-V guitar-gun** (`spawn_guitar_gun` + `carry_guitar`): a
    free-standing primitive prop carried at the hands each frame, neck +
    glowing muzzle down the aim — doubles as an aim indicator, tucked away
    mid-roll.
  - **Cyan ground ring** (`HeroRing`, emissive torus) + a warm `PointLight`,
    both parented to the player, so he's never lost in the swarm.
  - Player scale 1.85 → 2.0. Capture bot dodge-rolls far less (was tumbling
    constantly on camera).
- **2026-09-02**: Skins + face-cube enemies (all committed, verified in capture).
  - **Skin pipeline** = recolour a stock Kenney skin, not paint blind.
    `tools/gen_skins.py` takes `Textures/texture-p.png` (the businessman skin —
    already has the cube UV layout, a suit, and moulded shades) and remaps its
    suit / hair / shirt regions by colour-distance mask, keeping per-texel
    shading. Outputs `assets/models/blocky/skins/{hero,goon}.png`.
    `on_model_ready` (`player.rs`) swaps the atlas onto every body-part material
    via `PendingSkin`. Player uses `hero`; `goon` is generated for later.
    - Taste call: `hero` suit is charcoal `#222228`, not the spec's near-black
      `#0A0A0C` — pure black was an unreadable mud-lump at the top-down camera.
      Hair stays true black. Player scale 1.6 → 1.85. Revisit if wrong.
  - **Face-cubes** (`enemy.rs`): all 3 enemy kinds are now a solid-hue `Cuboid` +
    a camera-facing face-decal quad (`tools/gen_faces.py` paints the faces:
    green rusher / purple sponge / red flyer, angry blocky features). The parent
    stays axis-aligned so the decal always reads; only the `Body` child spins
    (flyer) or turns to the player. Tripo `rusher/sponge/flyer.glb` unwired,
    files kept. `ImagePlugin::default_nearest()` added to `lib.rs` for crisp
    texels.
  - Separation steering reworked (`move_enemies`): push is now an independent
    velocity term (clamped) added to the seek, not folded into a normalized
    dir — a tight crowd now shoves itself apart. Enemy cap 95 → 55, sponge
    smaller.

### Next steps

**Co-op rework.** M1 graphics + town, M2 co-op + drummer, M3 build phase + audio,
M4 the level loop, M5 coverage feedback (minimap + gloom→lit grade) are all
**done** (see build log). Next is **M6** (LAN). The standing polish backlog:

1. **Balance pass** (owed since session 3). Tune per-wave `wave_shape` budgets /
   scrap costs (speaker 18, stage 60) / channel times; rein in the combined
   pickup + beat-AoE + loudspeaker-fire over-clear; give the drummer its own
   pickups (they all buff the guitarist right now).
2. Audio polish: the count-in should musically *lead* the chord; mix levels;
   real stems instead of the numpy-synth placeholders.
3. Enemy faces are placeholder cubes (the "bad vibe" — see *Story*); revisit
   once the retheme lands. Goon: cigarette prop, a `die`/stagger clip, a slow
   melee wind-up.
4. City polish: proper AABB collision instead of the circle approximation;
   flow-field / coarse-grid pathing if local-avoidance stalls show; the couple
   of terracotta suburban houses that read as furniture from top-down.
5. On-screen buff / structure indicators (active pickup + timer, amp lifetime,
   stage channel progress) — the build ring is the only current cue.
