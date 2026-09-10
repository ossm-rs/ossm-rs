use core::sync::atomic::{AtomicBool, Ordering};

use log::{error, info};
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::{Duration, Instant, Ticker};
use esp_radio::esp_now::{
    EspNowManager, EspNowReceiver, EspNowSender, PeerInfo, BROADCAST_ADDRESS,
};
use portable_atomic::AtomicU64;
use zerocopy::{Immutable, IntoBytes, KnownLayout, TryFromBytes};

use crate::config::{MAX_NO_REMOTE_HEARTBEAT_MS, MAX_TRAVEL_MM, MOTION_CONTROL_MAX_VELOCITY};

use ossm_motion::motion::motion_state::{
    set_motion_depth_mm, set_motion_enabled, set_motion_length_mm, set_motion_pattern,
    set_motion_sensation_neg_pos_100, set_motion_velocity_mm_s,
};

const OSSM_ID: i32 = 1;
const M5_ID: i32 = 99;

static LAST_HEARTBEAT: AtomicU64 = AtomicU64::new(0);
static CONNECTED: AtomicBool = AtomicBool::new(false);

#[derive(Default, Debug, TryFromBytes, IntoBytes, Immutable)]
#[repr(i32)]
// The commands are not constructed
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
    fn heartbeat_packet() -> Self {
        Self {
            connected: true,
            target: M5_ID,
            speed: MOTION_CONTROL_MAX_VELOCITY as f32,
            depth: MAX_TRAVEL_MM as f32,
            ..Default::default()
        }
    }
}

async fn send_heartbeat_packet(
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    peer: &PeerInfo,
) {
    let mut sender = sender.lock().await;
    if let Err(err) = sender
        .send_async(&peer.peer_address, M5Packet::heartbeat_packet().as_bytes())
        .await
    {
        error!("Could not send the heartbeat packet {}", err);
    }
}

/// Task to get the motor packets and update the state
#[embassy_executor::task]
pub async fn m5_task(
    manager: &'static EspNowManager<'static>,
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    mut receiver: EspNowReceiver<'static>,
) {
    info!("Task M5 Listener Started");

    loop {
        let r = receiver.receive_async().await;
        // info!("Received {:?}", r);

        let data = r.data();
        let packet = match M5Packet::try_ref_from_bytes(data) {
            Ok(packet) => packet,
            Err(err) => {
                error!(
                    "Failed to parse the M5 Packet {:?}",
                    err
                );
                continue;
            }
        };

        if let M5Command::Heartbeat = packet.command {
        } else {
            info!("M5 Packet {:?}", packet);
        }

        match packet.command {
            M5Command::On => {
                let packet = M5Packet {
                    target: M5_ID,
                    command: M5Command::On,
                    ..Default::default()
                };
                let peer = manager
                    .fetch_peer(true)
                    .expect("Peer not found even though packet received");
                let mut sender = sender.lock().await;
                sender
                    .send_async(&peer.peer_address, packet.as_bytes())
                    .await
                    .expect("Could not send ON packet");
                set_motion_enabled(true);
            }
            M5Command::Off => {
                let packet = M5Packet {
                    target: M5_ID,
                    command: M5Command::Off,
                    ..Default::default()
                };
                let peer = manager
                    .fetch_peer(true)
                    .expect("Peer not found even though packet received");
                let mut sender = sender.lock().await;
                sender
                    .send_async(&peer.peer_address, packet.as_bytes())
                    .await
                    .expect("Could not send OFF packet");
                set_motion_enabled(false);
            }
            M5Command::Speed => {
                set_motion_velocity_mm_s(packet.value as u32);
            }
            M5Command::Depth => {
                set_motion_depth_mm(packet.value as u32);
            }
            M5Command::Stroke => {
                set_motion_length_mm(packet.value as u32);
            }
            M5Command::Sensation => {
                set_motion_sensation_neg_pos_100(packet.value as i32);
            }
            M5Command::Pattern => {
                set_motion_pattern(packet.value as u32);
            }
            M5Command::Heartbeat => {
                let now = Instant::now().as_millis();
                LAST_HEARTBEAT.store(now, Ordering::Release);
            }
            _ => {}
        }

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

            // Signal that we are paired
            send_heartbeat_packet(sender, &peer).await;
        }
    }
}

/// Task to check the heartbeats from the remote
/// and shut the machine off
#[embassy_executor::task]
pub async fn m5_heartbeat_check_task() {
    info!("Task M5 Heartbeat Check Started");

    let mut ticker = Ticker::every(Duration::from_millis(1000));
    loop {
        let last_heartbeat = Instant::from_millis(LAST_HEARTBEAT.load(Ordering::Acquire));
        let elapsed = last_heartbeat.elapsed().as_millis();

        CONNECTED.store(elapsed <= MAX_NO_REMOTE_HEARTBEAT_MS, Ordering::Release);

        ticker.next().await;
    }
}

/// Task to send heartbeats to the remote
#[embassy_executor::task]
pub async fn m5_heartbeat_task(
    manager: &'static EspNowManager<'static>,
    sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
) {
    info!("Task M5 Heartbeat Started");

    let mut ticker = Ticker::every(Duration::from_millis(5000));

    loop {
        ticker.next().await;

        let peer = match manager.fetch_peer(true) {
            Ok(peer) => peer,
            Err(_err) => {
                // Peer not found
                continue;
            }
        };

        send_heartbeat_packet(sender, &peer).await;
    }
}

pub fn is_m5_connected() -> bool {
    CONNECTED.load(Ordering::Acquire)
}
