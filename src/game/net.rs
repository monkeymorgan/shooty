//! M6 — LAN co-op via a thin snapshot client.
//!
//! Host-authoritative: the **host** runs the whole game exactly as the local
//! build does, and ~30x/s serialises a snapshot of the visible world (both
//! heroes, the live enemies, the secured zones, the HUD counters) into one UDP
//! datagram. A **client** runs the same binary with every *authoritative*
//! system switched off — no spawner, no combat resolution, no build channel —
//! and instead rebuilds that world from the snapshot stream each frame,
//! rendering "ghost" entities with the real models. The client sends its own
//! hero's [`Intent`] back so the host can drive it.
//!
//! No matchmaking, no netcode crate — a plain non-blocking [`UdpSocket`] polled
//! once per frame. Host is the **guitarist**, client is the **drummer**.
//!
//!   shooty                 local shared-screen co-op (unchanged)
//!   shooty host [port]      run the game and accept one LAN client
//!   shooty join <addr>      join a host (e.g. `shooty join 192.168.1.20`)

use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    Downed, GameState, Health, Hero, Player, Score,
    build::{Scrap, SecuredZones, SpeakerNet, Zone},
    enemy::{Wave, WavePhase},
    ground,
    player::{Aim, Intent, Moving},
};

pub const DEFAULT_PORT: u16 = 47474;
/// Snapshot cadence — the host sends at most this often.
const SNAPSHOT_DT: f32 = 1.0 / 30.0;
/// Datagram scratch buffer. LAN MTU will fragment a big one but reassemble it;
/// fine for a prototype.
const MAX_DATAGRAM: usize = 32 * 1024;

// ---------------------------------------------------------------------------
// Role
// ---------------------------------------------------------------------------

/// Which side of the connection this process is. `Local` is the default (no
/// networking); the run-condition helpers below read this.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NetRole {
    #[default]
    Local,
    Host,
    Client,
}

/// Run condition: this process owns the simulation (everything except a
/// connected client). Gate spawners / combat / build / weapons on this.
pub fn authoritative(role: Res<NetRole>) -> bool {
    *role != NetRole::Client
}

/// Run condition: this process is the host of a net game.
pub fn is_host(role: Res<NetRole>) -> bool {
    *role == NetRole::Host
}

/// Run condition: this process is a client rendering a remote game.
pub fn is_client(role: Res<NetRole>) -> bool {
    *role == NetRole::Client
}

/// How the process was launched — parsed from argv by [`launch_from_args`].
pub enum Launch {
    /// `shooty` — single hero, no networking.
    Solo,
    /// `shooty coop` — two heroes, local shared screen.
    Coop,
    Host(u16),
    Client(SocketAddr),
}

/// `shooty [coop | host [port] | join <addr>]`.
pub fn launch_from_args() -> Launch {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("coop") => Launch::Coop,
        Some("host") => {
            let port = args
                .get(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_PORT);
            Launch::Host(port)
        }
        Some("join") => {
            let raw = args.get(1).cloned().unwrap_or_else(|| {
                eprintln!("usage: shooty join <addr>");
                std::process::exit(2);
            });
            // Accept a bare IP (default port) or ip:port.
            let with_port = if raw.contains(':') {
                raw
            } else {
                format!("{raw}:{DEFAULT_PORT}")
            };
            let addr = with_port
                .to_socket_addrs()
                .ok()
                .and_then(|mut it| it.next())
                .unwrap_or_else(|| {
                    eprintln!("could not resolve host address `{with_port}`");
                    std::process::exit(2);
                });
            Launch::Client(addr)
        }
        _ => Launch::Solo,
    }
}

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default, Clone, Copy)]
pub struct HeroWire {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub aim: [f32; 2],
    pub hp: f32,
    pub hp_max: f32,
    pub downed: bool,
    pub moving: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct EnemyWire {
    pub id: u32,
    pub pos: [f32; 3],
    pub yaw: f32,
    /// `EnemyKind as u8`.
    pub kind: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct ZoneWire {
    pub x: f32,
    pub y: f32,
    pub r: f32,
}

/// One host → client frame.
#[derive(Serialize, Deserialize, Default)]
pub struct Snapshot {
    pub tick: u32,
    /// `GameState`: 0 Playing, 1 GameOver, 2 Victory.
    pub state: u8,
    pub guitarist: HeroWire,
    pub drummer: HeroWire,
    pub enemies: Vec<EnemyWire>,
    pub zones: Vec<ZoneWire>,
    pub kills: u32,
    pub scrap: u32,
    pub wave_number: u32,
    /// `WavePhase`: 0 Prep, 1 Spawning, 2 Clearing.
    pub wave_phase: u8,
    pub prep_left: f32,
    pub run_clock: f32,
    pub speakers_linked: u8,
}

/// One client → host frame: the drummer's intent for this tick.
#[derive(Serialize, Deserialize, Default)]
pub struct InputFrame {
    pub move_dir: [f32; 2],
    pub aim: [f32; 2],
    pub fire: bool,
    pub attack: bool,
    pub special: bool,
    pub dodge: bool,
    pub build: bool,
}

fn state_to_u8(s: GameState) -> u8 {
    match s {
        GameState::Playing => 0,
        GameState::GameOver => 1,
        GameState::Victory => 2,
        // A client never sits on the select screen — it joins a run in
        // progress — so it follows the host straight into Playing.
        GameState::Select => 0,
    }
}
fn u8_to_state(v: u8) -> GameState {
    match v {
        1 => GameState::GameOver,
        2 => GameState::Victory,
        _ => GameState::Playing,
    }
}
fn phase_to_u8(p: WavePhase) -> u8 {
    match p {
        WavePhase::Prep => 0,
        WavePhase::Spawning => 1,
        WavePhase::Clearing => 2,
    }
}
pub fn u8_to_phase(v: u8) -> WavePhase {
    match v {
        1 => WavePhase::Spawning,
        2 => WavePhase::Clearing,
        _ => WavePhase::Prep,
    }
}

fn yaw_of(t: &Transform) -> f32 {
    t.rotation.to_euler(EulerRot::YXZ).0
}

// ---------------------------------------------------------------------------
// Shared resources
// ---------------------------------------------------------------------------

/// Non-blocking socket + the peer we're talking to.
#[derive(Resource)]
struct Socket {
    udp: UdpSocket,
    /// Host: learned from the client's first packet. Client: the server.
    peer: Option<SocketAddr>,
    buf: Vec<u8>,
}

/// Host side: the latest drummer intent received from the client, with the
/// edge-triggered actions OR-accumulated until a sim tick consumes them.
#[derive(Resource, Default)]
pub struct RemoteInput {
    pub frame: InputFrame,
    pub connected: bool,
}

/// Client side: the most recent decoded snapshot, applied every frame.
#[derive(Resource, Default)]
pub struct LatestSnapshot(pub Option<Snapshot>);

/// Client side: `enemy net id -> ghost entity`.
#[derive(Resource, Default)]
pub struct EnemyGhosts(pub HashMap<u32, Entity>);

/// Client side: how many secured zones we've already raised a structure ghost
/// for (the host's zone list is append-only).
#[derive(Resource, Default)]
pub struct ZonesRealised(pub usize);

/// Host side: monotonically increasing id source for replicated enemies.
#[derive(Resource, Default)]
pub struct NetIds(u32);

impl NetIds {
    fn next(&mut self) -> u32 {
        self.0 += 1;
        self.0
    }
}

/// On a host-side enemy: its replication id.
#[derive(Component, Clone, Copy)]
pub struct NetId(pub u32);

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct NetPlugin {
    role: NetRole,
    port: u16,
    server: Option<SocketAddr>,
}

impl NetPlugin {
    pub fn host(port: u16) -> Self {
        Self {
            role: NetRole::Host,
            port,
            server: None,
        }
    }
    pub fn client(server: SocketAddr) -> Self {
        Self {
            role: NetRole::Client,
            port: 0,
            server: Some(server),
        }
    }
}

impl Plugin for NetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.role);

        match self.role {
            NetRole::Host => {
                let udp = UdpSocket::bind(("0.0.0.0", self.port))
                    .unwrap_or_else(|e| panic!("net: bind udp 0.0.0.0:{}: {e}", self.port));
                udp.set_nonblocking(true).expect("net: nonblocking");
                info!("net: hosting on udp/{} — waiting for a client", self.port);
                app.insert_resource(Socket {
                    udp,
                    peer: None,
                    buf: vec![0u8; MAX_DATAGRAM],
                })
                .init_resource::<RemoteInput>()
                .init_resource::<NetIds>()
                .add_systems(
                    PreUpdate,
                    (host_recv, assign_net_ids, apply_remote_intent).chain(),
                )
                .add_systems(PostUpdate, host_send);
            }
            NetRole::Client => {
                let server = self.server.expect("client role without a server addr");
                let udp = UdpSocket::bind("0.0.0.0:0").expect("net: bind ephemeral udp");
                udp.set_nonblocking(true).expect("net: nonblocking");
                // A hello so the host learns our address before its first send.
                let _ = udp.send_to(&[0u8], server);
                info!("net: joining {server}");
                app.insert_resource(Socket {
                    udp,
                    peer: Some(server),
                    buf: vec![0u8; MAX_DATAGRAM],
                })
                .init_resource::<LatestSnapshot>()
                .init_resource::<EnemyGhosts>()
                .init_resource::<ZonesRealised>()
                .add_systems(PreUpdate, client_recv)
                // Not state-gated: this is what *drives* the client's state, so
                // it has to keep running through GameOver / Victory to see the
                // host restart.
                .add_systems(Update, apply_snapshot_state)
                .add_systems(
                    PostUpdate,
                    apply_snapshot_transforms.before(TransformSystems::Propagate),
                )
                .add_systems(PostUpdate, client_send_input)
                .add_systems(
                    OnEnter(GameState::Playing),
                    |mut g: ResMut<EnemyGhosts>, mut z: ResMut<ZonesRealised>| {
                        g.0.clear();
                        z.0 = 0;
                    },
                );
            }
            NetRole::Local => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Host
// ---------------------------------------------------------------------------

/// Drain the socket; keep the newest input frame, OR-in the edge actions so a
/// press isn't lost to a frame-rate mismatch.
fn host_recv(mut sock: ResMut<Socket>, mut remote: ResMut<RemoteInput>) {
    let mut buf = std::mem::take(&mut sock.buf);
    loop {
        match sock.udp.recv_from(&mut buf) {
            Ok((n, from)) => {
                if !remote.connected || sock.peer != Some(from) {
                    info!("net: client connected from {from}");
                }
                sock.peer = Some(from);
                remote.connected = true;
                if n <= 1 {
                    continue; // hello / keep-alive
                }
                if let Ok(frame) = bincode::deserialize::<InputFrame>(&buf[..n]) {
                    let acc = &mut remote.frame;
                    acc.move_dir = frame.move_dir;
                    acc.aim = frame.aim;
                    acc.fire = frame.fire;
                    acc.attack |= frame.attack;
                    acc.special |= frame.special;
                    acc.dodge |= frame.dodge;
                    acc.build |= frame.build;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
    sock.buf = buf;
}

/// Tag any freshly-spawned enemy with a replication id.
fn assign_net_ids(
    mut commands: Commands,
    mut ids: ResMut<NetIds>,
    fresh: Query<Entity, (With<super::Enemy>, Without<NetId>)>,
) {
    for e in &fresh {
        commands.entity(e).insert(NetId(ids.next()));
    }
}

/// Feed the client's drummer intent onto the host's drummer entity, then clear
/// the accumulated edges.
fn apply_remote_intent(
    mut remote: ResMut<RemoteInput>,
    mut q: Query<(&mut Intent, &mut Aim, &Hero), With<Player>>,
) {
    if !remote.connected {
        return;
    }
    for (mut intent, mut aim, hero) in &mut q {
        if *hero != Hero::Drummer {
            continue;
        }
        let f = &remote.frame;
        intent.move_dir = Vec2::from(f.move_dir);
        intent.fire = f.fire;
        intent.attack = f.attack;
        intent.special = f.special;
        intent.dodge = f.dodge;
        intent.build = f.build;
        let a = Vec2::from(f.aim);
        if a != Vec2::ZERO {
            aim.0 = ground(a.normalize_or_zero(), 0.0);
        }
    }
    let f = &mut remote.frame;
    f.attack = false;
    f.special = false;
    f.dodge = false;
    f.build = false;
}

#[allow(clippy::too_many_arguments)]
fn host_send(
    time: Res<Time>,
    mut acc: Local<f32>,
    sock: Res<Socket>,
    state: Res<State<GameState>>,
    score: Res<Score>,
    scrap: Option<Res<Scrap>>,
    wave: Option<Res<Wave>>,
    zones: Option<Res<SecuredZones>>,
    speakers: Option<Res<SpeakerNet>>,
    clock: Option<Res<super::hud::RunClock>>,
    heroes: Query<(&Transform, &Aim, &Health, &Moving, Option<&Downed>, &Hero), With<Player>>,
    enemies: Query<(&NetId, &Transform, &super::Enemy)>,
) {
    let Some(peer) = sock.peer else { return };

    *acc += time.delta_secs();
    if *acc < SNAPSHOT_DT {
        return;
    }
    *acc = 0.0;

    let hero_wire = |want: Hero| -> HeroWire {
        heroes
            .iter()
            .find(|(.., h)| **h == want)
            .map(|(t, aim, hp, moving, downed, _)| HeroWire {
                pos: t.translation.to_array(),
                yaw: yaw_of(t),
                aim: [aim.0.x, aim.0.z],
                hp: hp.current,
                hp_max: hp.max,
                downed: downed.is_some(),
                moving: moving.0,
            })
            .unwrap_or_default()
    };

    let snap = Snapshot {
        tick: {
            static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        },
        state: state_to_u8(*state.get()),
        guitarist: hero_wire(Hero::Guitarist),
        drummer: hero_wire(Hero::Drummer),
        enemies: enemies
            .iter()
            .map(|(id, t, e)| EnemyWire {
                id: id.0,
                pos: t.translation.to_array(),
                yaw: yaw_of(t),
                kind: e.kind as u8,
            })
            .collect(),
        zones: zones
            .as_deref()
            .map(|z| {
                z.0.iter()
                    .map(|z| ZoneWire {
                        x: z.center.x,
                        y: z.center.y,
                        r: z.radius,
                    })
                    .collect()
            })
            .unwrap_or_default(),
        kills: score.kills,
        scrap: scrap.map(|s| s.0).unwrap_or(0),
        wave_number: wave.as_deref().map(|w| w.number).unwrap_or(0),
        wave_phase: wave.as_deref().map(|w| phase_to_u8(w.phase)).unwrap_or(0),
        prep_left: wave.as_deref().map(|w| w.prep_left()).unwrap_or(0.0),
        run_clock: clock.map(|c| c.0).unwrap_or(0.0),
        speakers_linked: speakers.map(|s| s.largest as u8).unwrap_or(0),
    };

    if let Ok(bytes) = bincode::serialize(&snap) {
        let _ = sock.udp.send_to(&bytes, peer);
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

fn client_recv(mut sock: ResMut<Socket>, mut latest: ResMut<LatestSnapshot>) {
    let mut buf = std::mem::take(&mut sock.buf);
    let mut newest: Option<Snapshot> = None;
    loop {
        match sock.udp.recv_from(&mut buf) {
            Ok((n, _)) => {
                if let Ok(snap) = bincode::deserialize::<Snapshot>(&buf[..n]) {
                    match &newest {
                        Some(prev) if prev.tick > snap.tick => {}
                        _ => newest = Some(snap),
                    }
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }
    sock.buf = buf;
    if let Some(snap) = newest {
        if latest.0.is_none() {
            info!(
                "net: receiving snapshots (tick {}, {} enemies)",
                snap.tick,
                snap.enemies.len()
            );
        }
        latest.0 = Some(snap);
    }
}

/// Send the local drummer's intent to the host every frame.
fn client_send_input(sock: Res<Socket>, q: Query<(&Intent, &Aim, &Hero), With<Player>>) {
    let Some(server) = sock.peer else { return };
    let Some((intent, aim, _)) = q.iter().find(|(.., h)| **h == Hero::Drummer) else {
        return;
    };
    let frame = InputFrame {
        move_dir: intent.move_dir.to_array(),
        aim: [aim.0.x, aim.0.z],
        fire: intent.fire,
        attack: intent.attack,
        special: intent.special,
        dodge: intent.dodge,
        build: intent.build,
    };
    if let Ok(bytes) = bincode::serialize(&frame) {
        let _ = sock.udp.send_to(&bytes, server);
    }
}

/// Non-transform snapshot application: game state, HUD counters, secured zones.
#[allow(clippy::too_many_arguments)]
fn apply_snapshot_state(
    latest: Res<LatestSnapshot>,
    state: Res<State<GameState>>,
    mut next_state: ResMut<NextState<GameState>>,
    mut score: ResMut<Score>,
    scrap: Option<ResMut<Scrap>>,
    wave: Option<ResMut<Wave>>,
    speakers: Option<ResMut<SpeakerNet>>,
    zones: Option<ResMut<SecuredZones>>,
    clock: Option<ResMut<super::hud::RunClock>>,
) {
    let Some(snap) = &latest.0 else { return };

    let want = u8_to_state(snap.state);
    if want != *state.get() {
        next_state.set(want);
    }

    score.kills = snap.kills;
    if let Some(mut s) = scrap {
        s.0 = snap.scrap;
    }
    if let Some(mut c) = clock {
        c.0 = snap.run_clock;
    }
    if let Some(mut n) = speakers {
        n.largest = snap.speakers_linked as usize;
    }
    if let Some(mut w) = wave {
        w.net_apply(
            snap.wave_number,
            u8_to_phase(snap.wave_phase),
            snap.prep_left,
        );
    }
    if let Some(mut z) = zones {
        z.0 = snap
            .zones
            .iter()
            .map(|z| Zone {
                center: Vec2::new(z.x, z.y),
                radius: z.r,
            })
            .collect();
    }
}

/// Transform application, in `PostUpdate` so it wins over the local `movement`
/// system: hard-drive the ghost guitarist, soft-correct the predicted drummer.
fn apply_snapshot_transforms(
    mut commands: Commands,
    latest: Res<LatestSnapshot>,
    mut heroes: Query<
        (
            Entity,
            &Hero,
            &mut Transform,
            &mut Aim,
            &mut Health,
            &mut Moving,
            Option<&Downed>,
        ),
        With<Player>,
    >,
) {
    let Some(snap) = &latest.0 else { return };

    for (entity, hero, mut t, mut aim, mut hp, mut moving, downed) in &mut heroes {
        let w = match hero {
            Hero::Guitarist => &snap.guitarist,
            Hero::Drummer => &snap.drummer,
        };
        if w.hp_max > 0.0 {
            hp.current = w.hp;
            hp.max = w.hp_max;
        }
        match (w.downed, downed.is_some()) {
            (true, false) => {
                commands.entity(entity).insert(Downed);
            }
            (false, true) => {
                commands.entity(entity).remove::<Downed>();
            }
            _ => {}
        }

        let snap_pos = Vec3::from(w.pos);
        match hero {
            Hero::Guitarist => {
                t.translation = snap_pos;
                t.rotation = Quat::from_rotation_y(w.yaw);
                aim.0 = ground(Vec2::new(w.aim[0], w.aim[1]).normalize_or_zero(), 0.0);
                moving.0 = w.moving;
            }
            Hero::Drummer => {
                // Locally predicted — only rein it in when it has drifted.
                if t.translation.distance(snap_pos) > 3.0 {
                    t.translation = snap_pos;
                }
            }
        }
    }
}
