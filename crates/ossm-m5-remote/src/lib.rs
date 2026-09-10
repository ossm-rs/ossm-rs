#![no_std]

use core::sync::atomic::{AtomicBool, Ordering};

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Ticker};
use esp_radio::esp_now::{
    BROADCAST_ADDRESS, EspNowManager, EspNowReceiver, EspNowSender, PeerInfo,
};
use log::{error, info};
use pattern_engine::PatternSender;
use portable_atomic::{AtomicU32, AtomicU64};
use zerocopy::{Immutable, IntoBytes, KnownLayout, TryFromBytes};

const OSSM_ID: i32 = 1;
const M5_ID: i32 = 99;
const MAX_NO_REMOTE_HEARTBEAT_MS: u64 = 10_000;

static LAST_HEARTBEAT: AtomicU64 = AtomicU64::new(0);
static CONNECTED: AtomicBool = AtomicBool::new(false);
static CURRENT_PATTERN_IDX: AtomicU32 = AtomicU32::new(0);

// The M5 remote has a hardcoded pattern list that differs in order (and
// contents) from the pattern engine's `AnyPattern::all_builtin()`.

/// The pattern list as it appears on the M5 remote's UI roller.
///
/// Variants without an engine equivalent return `None` from
/// [`Self::to_engine_index`].
#[derive(Debug, Clone, Copy)]
enum RemotePattern {
    SimpleStroke,
    TeasingPounding,
    RoboStroke,
    HalfnHalf,
    Deeper,
    StopNGo,
    Insist,
    Knot,
}

impl RemotePattern {
    fn from_remote_index(idx: u32) -> Option<Self> {
        match idx {
            0 => Some(Self::SimpleStroke),
            1 => Some(Self::TeasingPounding),
            2 => Some(Self::RoboStroke),
            3 => Some(Self::HalfnHalf),
            4 => Some(Self::Deeper),
            5 => Some(Self::StopNGo),
            6 => Some(Self::Insist),
            7 => Some(Self::Knot),
            _ => None,
        }
    }

    fn from_engine_index(idx: u32) -> Option<Self> {
        match idx {
            0 => Some(Self::SimpleStroke),
            1 => Some(Self::Deeper),
            2 => Some(Self::HalfnHalf),
            3 => Some(Self::StopNGo),
            4 => Some(Self::TeasingPounding),
            _ => None, // Torque (5), None (6) have no remote equivalent
        }
    }

    /// Convert to an engine pattern index.
    ///
    /// Patterns without an engine implementation map to the None pattern
    /// (index 6), which holds position.
    fn to_engine_index(self) -> u32 {
        match self {
            Self::SimpleStroke => 0,
            Self::Deeper => 1,
            Self::HalfnHalf => 2,
            Self::StopNGo => 3,
            Self::TeasingPounding => 4,
            Self::RoboStroke | Self::Insist | Self::Knot => 6, // None pattern
        }
    }

    fn to_remote_index(self) -> u32 {
        match self {
            Self::SimpleStroke => 0,
            Self::TeasingPounding => 1,
            Self::RoboStroke => 2,
            Self::HalfnHalf => 3,
            Self::Deeper => 4,
            Self::StopNGo => 5,
            Self::Insist => 6,
            Self::Knot => 7,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RemoteConfig {
    pub max_velocity_mm_s: f64,
    pub max_travel_mm: f64,
}

#[derive(Default, Debug, TryFromBytes, IntoBytes, Immutable)]
#[repr(i32)]
#[allow(dead_code)]
enum M5Command {
    Conn = 0,
    Speed = 1,
    Depth = 2,
    Stroke = 3,
    Sensation = 4,
    Pattern = 5,
    TorqueF = 6,
    TorqueR = 7,
    Off = 10,
    On = 11,
    SetupDI = 12,
    SetupDIF = 13,
    Reboot = 14,

    CumSpeed = 20,
    CumTime = 21,
    CumSize = 22,
    CumAccel = 23,

    Connect = 88,

    #[default]
    Heartbeat = 99,
}

#[derive(Default, Debug, TryFromBytes, IntoBytes, Immutable, KnownLayout)]
#[repr(C)]
struct M5Packet {
    speed: f32,
    depth: f32,
    stroke: f32,
    sensation: f32,
    pattern: f32,
    rstate: bool,
    connected: bool,
    heartbeat: bool,
    _padding: bool,
    command: M5Command,
    value: f32,
    target: i32,
}

impl M5Packet {
    fn heartbeat_packet(config: RemoteConfig) -> Self {
        let engine_idx = CURRENT_PATTERN_IDX.load(Ordering::Acquire);
        let remote_idx = RemotePattern::from_engine_index(engine_idx)
            .map(|p| p.to_remote_index())
            .unwrap_or(0);
        Self {
            connected: true,
            target: M5_ID,
            speed: config.max_velocity_mm_s as f32,
            depth: config.max_travel_mm as f32,
            pattern: remote_idx as f32,
            ..Default::default()
        }
    }
}

async fn send_heartbeat_packet(
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    peer: &PeerInfo,
    config: RemoteConfig,
) {
    let mut sender = sender.lock().await;
    if let Err(err) = sender
        .send_async(
            &peer.peer_address,
            M5Packet::heartbeat_packet(config).as_bytes(),
        )
        .await
    {
        error!("Could not send heartbeat packet: {}", err);
    }
}

pub fn start(
    spawner: &Spawner,
    manager: &'static EspNowManager<'static>,
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    receiver: EspNowReceiver<'static>,
    patterns: &'static PatternSender,
    config: RemoteConfig,
) {
    spawner
        .spawn(receiver_task(manager, sender, receiver, patterns, config))
        .unwrap();
    spawner
        .spawn(heartbeat_send_task(manager, sender, config))
        .unwrap();
    spawner.spawn(heartbeat_check_task(patterns)).unwrap();

    info!("ESP-NOW remote tasks started, waiting for connection...");
}

#[embassy_executor::task]
async fn receiver_task(
    manager: &'static EspNowManager<'static>,
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    mut receiver: EspNowReceiver<'static>,
    patterns: &'static PatternSender,
    config: RemoteConfig,
) {
    info!("ESP-NOW receiver task started");

    loop {
        let r = receiver.receive_async().await;
        let data = r.data();

        let packet = match M5Packet::try_ref_from_bytes(data) {
            Ok(packet) => packet,
            Err(err) => {
                error!("Failed to parse M5 packet: {:?}", err);
                continue;
            }
        };

        match packet.command {
            M5Command::On => {
                let ack = M5Packet {
                    target: M5_ID,
                    command: M5Command::On,
                    ..Default::default()
                };
                if let Ok(peer) = manager.fetch_peer(true) {
                    let mut sender = sender.lock().await;
                    if let Err(err) = sender.send_async(&peer.peer_address, ack.as_bytes()).await {
                        error!("Could not send ON ack: {}", err);
                    }
                }
                let current = CURRENT_PATTERN_IDX.load(Ordering::Acquire) as usize;
                info!("Playing pattern {}", current);
                patterns.play(current);
            }
            M5Command::Off => {
                let ack = M5Packet {
                    target: M5_ID,
                    command: M5Command::Off,
                    ..Default::default()
                };
                if let Ok(peer) = manager.fetch_peer(true) {
                    let mut sender = sender.lock().await;
                    if let Err(err) = sender.send_async(&peer.peer_address, ack.as_bytes()).await {
                        error!("Could not send OFF ack: {}", err);
                    }
                }
                patterns.pause();
                info!("Paused");
            }
            M5Command::Speed => {
                let velocity = (packet.value as f64) / config.max_velocity_mm_s;
                patterns.set_speed(velocity);
            }
            M5Command::Depth => {
                let depth = (packet.value as f64) / config.max_travel_mm;
                patterns.set_depth(depth);
            }
            M5Command::Stroke => {
                let stroke = (packet.value as f64) / config.max_travel_mm;
                patterns.set_stroke(stroke);
            }
            M5Command::Sensation => {
                // Remote sends -100..100; pattern engine expects -1.0..1.0
                let sensation = (packet.value as f64) / 100.0;
                patterns.set_sensation(sensation);
            }
            M5Command::Pattern => {
                let remote_idx = packet.value as u32;
                if let Some(pattern) = RemotePattern::from_remote_index(remote_idx) {
                    let engine_idx = pattern.to_engine_index();
                    CURRENT_PATTERN_IDX.store(engine_idx, Ordering::Release);
                    info!("Switching to pattern {}", engine_idx);
                    patterns.play(engine_idx as usize);
                }
            }
            M5Command::Heartbeat => {
                let now = Instant::now().as_millis();
                LAST_HEARTBEAT.store(now, Ordering::Release);
            }
            _ => {}
        }

        // Auto-pairing: if this packet is targeted at us via broadcast from
        // an unknown peer, register them and send a heartbeat to confirm.
        if packet.target == OSSM_ID
            && r.info.dst_address == BROADCAST_ADDRESS
            && !manager.peer_exists(&r.info.src_address)
        {
            let peer = PeerInfo {
                interface: esp_radio::esp_now::EspNowWifiInterface::Sta,
                peer_address: r.info.src_address,
                lmk: None,
                channel: None,
                encrypt: false,
            };
            manager.add_peer(peer).unwrap();
            info!("Added new peer {:?}", r.info.src_address);

            send_heartbeat_packet(sender, &peer, config).await;
        }
    }
}

#[embassy_executor::task]
async fn heartbeat_send_task(
    manager: &'static EspNowManager<'static>,
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    config: RemoteConfig,
) {
    info!("ESP-NOW heartbeat send task started");

    let mut ticker = Ticker::every(Duration::from_millis(5000));

    loop {
        ticker.next().await;

        let peer = match manager.fetch_peer(true) {
            Ok(peer) => peer,
            Err(_) => continue,
        };

        send_heartbeat_packet(sender, &peer, config).await;
    }
}

#[embassy_executor::task]
async fn heartbeat_check_task(patterns: &'static PatternSender) {
    info!("ESP-NOW heartbeat check task started");

    let mut ticker = Ticker::every(Duration::from_millis(1000));

    loop {
        ticker.next().await;

        let last_heartbeat_ms = LAST_HEARTBEAT.load(Ordering::Acquire);
        let was_connected = CONNECTED.load(Ordering::Acquire);
        let is_connected = last_heartbeat_ms > 0 && {
            let elapsed = Instant::from_millis(last_heartbeat_ms).elapsed().as_millis();
            elapsed <= MAX_NO_REMOTE_HEARTBEAT_MS
        };

        CONNECTED.store(is_connected, Ordering::Release);

        if was_connected && !is_connected {
            info!("Remote disconnected, heartbeat lost");
            patterns.pause();
        } else if !was_connected && is_connected {
            info!("Remote connected");
        }
    }
}

pub fn is_connected() -> bool {
    CONNECTED.load(Ordering::Acquire)
}

pub fn set_current_pattern(idx: u32) {
    CURRENT_PATTERN_IDX.store(idx, Ordering::Release);
}

pub fn current_pattern() -> u32 {
    CURRENT_PATTERN_IDX.load(Ordering::Acquire)
}
