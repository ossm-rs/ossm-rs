#![no_std]

use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};

pub const CONNECTIONS_MAX: usize = 1;
pub const L2CAP_CHANNELS_MAX: usize = 2;
pub const MAX_COMMAND_LENGTH: usize = 64;
pub const MAX_STATE_LENGTH: usize = 128;
pub const MAX_PATTERN_LENGTH: usize = 1024;
pub const MAX_XTOYS_LENGTH: usize = 240;

use embassy_executor::Spawner;
use embassy_futures::select::{Either, select};
use embassy_time::{Duration, Ticker, Timer};
use esp_radio::ble::controller::BleConnector;
use heapless::String;
use log::{error, info, warn};
use pattern_engine::{EngineState, PatternInput, PatternSender, commands};
use static_cell::StaticCell;
use trouble_host::prelude::*;

const SERVICE_UUID: Uuid = uuid!("522b443a-4f53-534d-0001-420badbabe69");
const PRIMARY_COMMAND_UUID: Uuid = uuid!("522b443a-4f53-534d-1000-420badbabe69");
const SPEED_KNOB_UUID: Uuid = uuid!("522b443a-4f53-534d-1010-420badbabe69");
const CURRENT_STATE_UUID: Uuid = uuid!("522b443a-4f53-534d-2000-420badbabe69");
const PATTERN_LIST_UUID: Uuid = uuid!("522b443a-4f53-534d-3000-420badbabe69");
const PATTERN_DESCRIPTION_UUID: Uuid = uuid!("522b443a-4f53-534d-3010-420badbabe69");
const XTOYS_SERVICE_UUID: Uuid = uuid!("e5560000-6a2d-436f-a43d-82eab88dcefd");
const XTOYS_CONTROL_UUID: Uuid = uuid!("e5560001-6a2d-436f-a43d-82eab88dcefd");

// Standard Device Information Service used by XToys Custom Firmware.
// denialtek firmware exposes both values as "2.0".
const DEVICE_INFO_SERVICE_UUID: Uuid = uuid!("0000180a-0000-1000-8000-00805f9b34fb");
const SOFTWARE_REVISION_UUID: Uuid = uuid!("00002a28-0000-1000-8000-00805f9b34fb");
const FIRMWARE_REVISION_UUID: Uuid = uuid!("00002a26-0000-1000-8000-00805f9b34fb");

static CONNECTED: AtomicBool = AtomicBool::new(false);
// XToys waits for an explicit home-result notification.
static XTOYS_HOME_PENDING: AtomicBool = AtomicBool::new(false);

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

#[gatt_server]
struct Server {
    ossm_service: OssmService,
    xtoys_service: XToysService,
    device_info_service: DeviceInfoService,
}

#[gatt_service(uuid = SERVICE_UUID)]
struct OssmService {
    #[characteristic(uuid = PRIMARY_COMMAND_UUID, read, write)]
    primary_command: String<MAX_COMMAND_LENGTH>,

    #[characteristic(uuid = SPEED_KNOB_UUID, read, write)]
    speed_knob: String<16>,

    #[characteristic(uuid = CURRENT_STATE_UUID, read, notify)]
    current_state: String<MAX_STATE_LENGTH>,

    #[characteristic(uuid = PATTERN_LIST_UUID, read)]
    pattern_list: String<MAX_PATTERN_LENGTH>,

    #[characteristic(uuid = PATTERN_DESCRIPTION_UUID, read, write)]
    pattern_description: String<MAX_PATTERN_LENGTH>,
}

#[gatt_service(uuid = DEVICE_INFO_SERVICE_UUID)]
struct DeviceInfoService {
    // denialtek/XToys-OSSM-Firmware:
    // 0x2A28 = API_VERSION "2.0"
    #[characteristic(uuid = SOFTWARE_REVISION_UUID, read)]
    software_revision: String<8>,

    // denialtek/XToys-OSSM-Firmware:
    // 0x2A26 = FIRMWARE_VERSION "2.0"
    #[characteristic(uuid = FIRMWARE_REVISION_UUID, read)]
    firmware_revision: String<8>,
}

#[gatt_service(uuid = XTOYS_SERVICE_UUID)]
struct XToysService {
    #[characteristic(uuid = XTOYS_CONTROL_UUID, read, write, notify)]
    control: String<MAX_XTOYS_LENGTH>,
}

fn get_all_patterns_json() -> String<MAX_PATTERN_LENGTH> {
    let mut output: String<MAX_PATTERN_LENGTH> = String::new();
    output.write_char('[').ok();
    for (i, meta) in commands::pattern_list().iter().enumerate() {
        let rollback = output.len();
        let sep = if i > 0 { "," } else { "" };
        if write!(output, r#"{sep}{{"name":"{}","idx":{i}}}"#, meta.name).is_err()
            || output.len() + 1 > output.capacity()
        {
            output.truncate(rollback);
            error!("Pattern list truncated at index {i}");
            break;
        }
    }
    output.write_char(']').ok();
    output
}

fn get_pattern_description(index: usize) -> String<MAX_PATTERN_LENGTH> {
    let mut output = String::new();

    let description = commands::pattern_description(index).unwrap_or("Invalid pattern index");

    if output.push_str(description).is_err() {
        output
            .push_str("Pattern Description Too Long")
            .expect("Always fits");
    }

    output
}

pub fn start(
    spawner: &Spawner,
    connector: BleConnector<'static>,
    patterns: &'static PatternSender,
) {
    let bt_controller: ExternalController<_, 20> = ExternalController::new(connector);

    let resources = mk_static!(HostResources<DefaultPacketPool, CONNECTIONS_MAX, L2CAP_CHANNELS_MAX>, HostResources::new());
    let stack = mk_static!(
        trouble_host::Stack<
            'static,
            ExternalController<BleConnector<'static>, 20>,
            DefaultPacketPool,
        >,
        trouble_host::new(bt_controller, resources)
    );

    let Host {
        peripheral, runner, ..
    } = stack.build();

    spawner.must_spawn(ble_runner_task(runner));
    spawner.must_spawn(ble_events_task(stack, peripheral, patterns));

    info!("BLE remote tasks started, waiting for connection...");
}

#[embassy_executor::task]
pub async fn ble_events_task(
    stack: &'static Stack<
        'static,
        ExternalController<BleConnector<'static>, 20>,
        DefaultPacketPool,
    >,
    mut peripheral: Peripheral<
        'static,
        ExternalController<BleConnector<'static>, 20>,
        DefaultPacketPool,
    >,
    patterns: &'static PatternSender,
) {
    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "OSSM",
        appearance: &appearance::motorized_device::GENERIC_MOTORIZED_DEVICE,
    }))
    .unwrap();

    // Match denialtek XToys Custom Firmware's BLE Device Information Service.
    let mut api_version: String<8> = String::new();
    api_version.push_str("2.0").expect("2.0 fits");
    server
        .set(&server.device_info_service.software_revision, &api_version)
        .expect("set XToys API version");

    let mut firmware_version: String<8> = String::new();
    firmware_version.push_str("2.0").expect("2.0 fits");
    server
        .set(
            &server.device_info_service.firmware_revision,
            &firmware_version,
        )
        .expect("set XToys firmware version");

    info!("XToys Device Information: API 2.0, firmware 2.0");

    loop {
        match advertise("OSSM", &mut peripheral).await {
            Ok(connection) => {
                CONNECTED.store(true, Ordering::Release);
                info!("BLE Connected");

                Timer::after_millis(100).await;

                if let Err(err) = connection.set_phy(stack, PhyKind::Le2M).await {
                    warn!("Failed to set 2M PHY, continuing with default: {:?}", err);
                }

                let connect_params = ConnectParams {
                    min_connection_interval: Duration::from_micros(7500),
                    max_connection_interval: Duration::from_micros(7500),
                    ..Default::default()
                };
                match connection
                    .update_connection_params(stack, &connect_params)
                    .await
                {
                    Ok(()) => info!("Connection interval set to 7.5ms"),
                    Err(err) => warn!(
                        "Failed to update connection params, continuing with defaults: {:?}",
                        err
                    ),
                }

                Timer::after_millis(100).await;

                match connection.read_phy(stack).await {
                    Ok(phy) => info!("PHY {:?} MTU {:?}", phy, connection.att_mtu()),
                    Err(err) => warn!(
                        "Could not read PHY: {:?}, MTU {:?}",
                        err,
                        connection.att_mtu()
                    ),
                };

                let gatt_connection = connection
                    .with_attribute_server(&server)
                    .expect("Could not transform connection into GATT connection");

                let events = gatt_events_task(&server, &gatt_connection, patterns);
                let notify = state_notifications(&server, &gatt_connection, patterns);

                match select(events, notify).await {
                    Either::First(res) => {
                        if let Err(err) = res {
                            error!("[gatt] error in events task: {:?}", err);
                        }
                    }
                    Either::Second(res) => match res {
                        Ok(()) => info!("[gatt] notify task ended cleanly"),
                        Err(err) => error!("[gatt] error in notify task: {:?}", err),
                    },
                }

                XTOYS_HOME_PENDING.store(false, Ordering::Release);
                patterns.stop();
                info!("BLE session ended, stopping engine");
            }
            Err(err) => {
                error!("[adv] error: {:?}", err);
            }
        }
    }
}

#[embassy_executor::task]
pub async fn ble_runner_task(
    mut runner: Runner<'static, ExternalController<BleConnector<'static>, 20>, DefaultPacketPool>,
) {
    loop {
        if let Err(err) = runner.run().await {
            error!("[ble_task] error: {:?}", err);
        }
    }
}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
    patterns: &'static PatternSender,
) -> Result<(), Error> {
    let reason = loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let mut write = false;
                let mut event_handle = 0;
                match &event {
                    GattEvent::Read(event) => {
                        if event.handle()
                            == server.device_info_service.software_revision.handle
                        {
                            info!("XToys read API version -> 2.0");
                        }

                        if event.handle()
                            == server.device_info_service.firmware_revision.handle
                        {
                            info!("XToys read firmware version -> 2.0");
                        }

                        if event.handle() == server.ossm_service.current_state.handle {
                            let engine_state = patterns.state();
                            let input = patterns.input();
                            let state_json = state_to_json(engine_state, &input);
                            server.set(&server.ossm_service.current_state, &state_json)?;
                        }
                        if event.handle() == server.ossm_service.pattern_list.handle {
                            let patterns = get_all_patterns_json();
                            server.set(&server.ossm_service.pattern_list, &patterns)?;
                        }
                    }
                    GattEvent::Write(event) => {
                        write = true;
                        event_handle = event.handle();
                    }
                    GattEvent::Other(_) => {}
                };
                // This step is also performed at drop(), but writing it explicitly is necessary
                // in order to ensure reply is sent.
                match event.accept() {
                    Ok(reply) => reply.send().await,
                    Err(e) => {
                        error!("[gatt] error sending response: {:?}", e);
                    }
                };

                // This is here because the event needs to be accepted before the data can be accessed
                if write {
                    if event_handle == server.ossm_service.primary_command.handle {
                        let command: String<MAX_COMMAND_LENGTH> =
                            server.get(&server.ossm_service.primary_command)?;
                        process_command(&command, server, patterns);
                    }
                    if event_handle == server.xtoys_service.control.handle {
                        let command: String<MAX_XTOYS_LENGTH> = server.get(&server.xtoys_service.control)?;
                        process_xtoys_message(command.as_str(), server, connection, patterns).await;
                    }
                    if event_handle == server.ossm_service.pattern_description.handle {
                        let command: String<MAX_PATTERN_LENGTH> =
                            server.get(&server.ossm_service.pattern_description)?;

                        let description = if let Ok(index) = command.parse::<usize>() {
                            get_pattern_description(index)
                        } else {
                            let mut description: String<MAX_PATTERN_LENGTH> = String::new();
                            description
                                .push_str("Could not parse pattern index")
                                .expect("Always fits");
                            description
                        };

                        server.set(&server.ossm_service.pattern_description, &description)?;
                    }
                }
            }
            GattConnectionEvent::PhyUpdated { .. }
            | GattConnectionEvent::ConnectionParamsUpdated { .. }
            | GattConnectionEvent::RequestConnectionParams { .. }
            | GattConnectionEvent::DataLengthUpdated { .. } => {}
        }
    };
    CONNECTED.store(false, Ordering::Release);
    info!("[gatt] disconnected: {:?}", reason);
    Ok(())
}

/// Create an advertiser to use to connect to a BLE Central, and wait for it to connect.
async fn advertise<'values, 'server, C: Controller>(
    name: &'values str,
    peripheral: &mut Peripheral<'values, C, DefaultPacketPool>,
) -> Result<Connection<'values, DefaultPacketPool>, BleHostError<C::Error>> {
    let ossm_uuid: [u8; 16] = SERVICE_UUID.as_raw().try_into().expect("OSSM service UUID incorrect");
    let xtoys_uuid: [u8; 16] = XTOYS_SERVICE_UUID.as_raw().try_into().expect("XToys service UUID incorrect");

    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[xtoys_uuid]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let mut scan_data = [0; 31];
    let scan_len = AdStructure::encode_slice(
        &[AdStructure::ServiceUuids128(&[ossm_uuid])],
        &mut scan_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &scan_data[..scan_len],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?;
    info!("[adv] connection established");
    Ok(conn)
}

async fn state_notifications<P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
    patterns: &'static PatternSender,
) -> Result<(), Error> {
    let mut sub = patterns
        .subscribe()
        .expect("No state subscriber slots available");
    let mut heartbeat = Ticker::every(Duration::from_secs(1));

    loop {
        let engine_state = match select(sub.next_message_pure(), heartbeat.next()).await {
            Either::First(state) => state,
            Either::Second(_) => patterns.state(),
        };

        let input = patterns.input();
        let state_json = state_to_json(engine_state, &input);
        // XToys Custom clients do not necessarily subscribe to the stock
        // OSSM-rs current-state characteristic. Failure to notify that
        // characteristic must not terminate the shared BLE notification task.
        if let Err(err) = server
            .ossm_service
            .current_state
            .notify(connection, &state_json)
            .await
        {
            warn!("OSSM state notify skipped/failed: {:?}", err);
        }

        // denialtek XToys firmware reports homing completion with exactly:
        // [{"action":"home","success":true}]
        //
        // Use a pending request flag rather than relying on observing the
        // exact Homing -> Ready transition, because state messages can be
        // coalesced while the physical homing sequence still succeeds.
        if XTOYS_HOME_PENDING.load(Ordering::Acquire)
            && engine_state == EngineState::Ready
        {
            info!("XToys home complete -> sending success notification");
            xtoys_notify(
                server,
                connection,
                r#"[{"action":"home","success":true}]"#,
            )
            .await;
            XTOYS_HOME_PENDING.store(false, Ordering::Release);
        }
    }
}

fn state_to_json(state: EngineState, input: &PatternInput) -> String<MAX_STATE_LENGTH> {
    let pattern_name = match state {
        EngineState::Playing(idx) | EngineState::Paused(idx) => commands::pattern_list()
            .get(idx)
            .map(|m| m.name)
            .unwrap_or(""),
        _ => "",
    };
    let mut out: String<MAX_STATE_LENGTH> = String::new();
    let state_str = match state {
        EngineState::Idle => "idle",
        EngineState::Homing => "homing",
        EngineState::Ready => "ready",
        EngineState::Playing(_) => "playing",
        EngineState::Paused(_) => "paused",
    };
    let idx = match state {
        EngineState::Playing(i) | EngineState::Paused(i) => i,
        _ => 0,
    };
    let speed = (input.velocity * 100.0) as u32;
    let stroke = (input.stroke * 100.0) as u32;
    let depth = (input.depth * 100.0) as u32;
    // Map internal -1.0..1.0 back to BLE protocol 0–100.
    let sensation = ((input.sensation + 1.0) * 50.0) as u32;
    let _ = write!(
        out,
        r#"{{"state":"{state_str}","speed":{speed},"stroke":{stroke},"sensation":{sensation},"depth":{depth},"pattern":{idx},"patternName":"{pattern_name}"}}"#,
    );
    out
}


fn json_find_value<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let mut key_buf: String<48> = String::new();
    let _ = write!(key_buf, "\"{}\"", key);
    let start = object.find(key_buf.as_str())?;
    let after = &object[start + key_buf.len()..];
    let colon = after.find(':')?;
    Some(after[colon + 1..].trim_start())
}
fn json_string<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let value = json_find_value(object, key)?.strip_prefix('"')?;
    let end = value.find('"')?;
    Some(&value[..end])
}
fn json_number(object: &str, key: &str) -> Option<f64> {
    let value = json_find_value(object, key)?;
    let end = value.find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '+' || c == '.')).unwrap_or(value.len());
    value[..end].parse::<f64>().ok()
}
fn json_bool(object: &str, key: &str) -> bool {
    json_find_value(object, key).map(|v| v.starts_with("true")).unwrap_or(false)
}
async fn xtoys_notify<P: PacketPool>(server: &Server<'_>, connection: &GattConnection<'_, '_, P>, value: &str) {
    let mut response: String<MAX_XTOYS_LENGTH> = String::new();
    if response.push_str(value).is_err() { return; }
    let _ = server.set(&server.xtoys_service.control, &response);
    if let Err(err) = server.xtoys_service.control.notify(connection, &response).await {
        warn!("XToys notify skipped/failed: {:?}", err);
    }
}
async fn process_xtoys_object<P: PacketPool>(object: &str, server: &Server<'_>, connection: &GattConnection<'_, '_, P>, patterns: &'static PatternSender) {
    let Some(action) = json_string(object, "action") else { return; };
    info!("XToys action {}", action);
    match action {
        "connected" => patterns.stop(),
        "home" => {
            XTOYS_HOME_PENDING.store(true, Ordering::Release);
            patterns.home();
        },
        "setConfig" => {
            let head = json_number(object, "head").unwrap_or(10.0).clamp(0.0, 100.0) / 100.0;
            let suck = json_number(object, "suck").unwrap_or(50.0).clamp(0.0, 100.0) / 100.0;
            let dt = json_number(object, "dt").unwrap_or(75.0).clamp(0.0, 100.0) / 100.0;
            let speed = json_number(object, "speed").unwrap_or(50.0).clamp(0.0, 100.0) / 100.0;
            let count = json_number(object, "count").unwrap_or(20.0).clamp(1.0, 10_000.0) as u32;
            let dt_every = json_number(object, "dtEvery").unwrap_or(5.0).clamp(0.0, 10_000.0) as u32;
            let dt_hold_ms = (json_number(object, "dtHold").unwrap_or(2.0).clamp(0.0, 60.0) * 1000.0) as u32;
            patterns.routine_configure(head, suck, dt, speed, count, dt_every, dt_hold_ms);
            xtoys_notify(server, connection, r#"[{"action":"setConfig","success":true}]"#).await;
        },
        "startRoutine" => {
            patterns.routine_start();
            xtoys_notify(server, connection, r#"[{"action":"startRoutine","success":true}]"#).await;
        },
        "stopRoutine" => {
            patterns.routine_stop();
            xtoys_notify(server, connection, r#"[{"action":"stopRoutine","success":true}]"#).await;
        },
        "setPattern" => patterns.play(json_number(object, "pattern").unwrap_or(0.0).max(0.0) as usize),
        "pause" => patterns.pause(),
        "resume" => patterns.resume(),
        "stop" => patterns.xtoys_stop(),
        "setSpeed" => patterns.set_speed(json_number(object, "speed").unwrap_or(0.0).clamp(0.0,100.0)/100.0),
        "setDepth" => patterns.set_depth(json_number(object, "depth").unwrap_or(0.0).clamp(0.0,100.0)/100.0),
        "setStroke" => patterns.set_stroke(json_number(object, "stroke").unwrap_or(0.0).clamp(0.0,100.0)/100.0),
        "setSensation" => patterns.set_sensation(json_number(object, "sensation").unwrap_or(0.0).clamp(-100.0,100.0)/100.0),
        "startStreaming" => patterns.start_streaming(),
        "move" => {
            // XToys emits an initialization packet when entering Position mode
            // with `"time":null`.  This is not a real timed move.  Treating it
            // as 0 ms feeds a full-speed Ruckig point-to-point command into the
            // streaming runner and can leave the runner waiting forever while
            // BLE itself remains connected.
            //
            // Keep legitimate numeric 0 ms commands intact (notably the
            // explicit retract/extend helpers below); only JSON null is ignored.
            let time_value = json_find_value(object, "time");
            if time_value.map(|v| v.starts_with("null")).unwrap_or(false) {
                info!("XToys Position-mode seed move ignored (time=null)");
            } else {
                patterns.stream_move(
                    json_number(object, "position").unwrap_or(0.0).clamp(0.0,100.0)/100.0,
                    json_number(object, "time").unwrap_or(0.0).clamp(0.0,u32::MAX as f64) as u32,
                    json_bool(object, "replace")
                );
            }
        },
        "disable" => {
            XTOYS_HOME_PENDING.store(false, Ordering::Release);
            patterns.stop();
        },
        "version" => xtoys_notify(server, connection, r#"[{"action":"version","api":"2.0","firmware":"2.0-ossm-rs"}]"#).await,
        "configureBluetooth" => xtoys_notify(server, connection, r#"[{"action":"configureBluetooth","success":true}]"#).await,
        "getPatternList" => xtoys_notify(server, connection, r#"[{"action":"getPatternList","patterns":[{"name":"Simple","idx":0}]}]"#).await,
        "setup" => patterns.stop(),
        "retract" => { patterns.start_streaming(); patterns.stream_move(0.0, 0, true); },
        "extend" => { patterns.start_streaming(); patterns.stream_move(1.0, 0, true); },
        "configureWebsocket" => warn!("XToys configureWebsocket ignored in Bluetooth build"),
        other => warn!("Unsupported XToys action {}", other),
    }
}
async fn process_xtoys_message<P: PacketPool>(message: &str, server: &Server<'_>, connection: &GattConnection<'_, '_, P>, patterns: &'static PatternSender) {
    info!("XToys JSON {}", message);
    let mut depth = 0usize;
    let mut start = None;
    for (idx, ch) in message.char_indices() {
        match ch {
            '{' => { if depth == 0 { start = Some(idx); } depth += 1; }
            '}' => { if depth > 0 { depth -= 1; if depth == 0 { if let Some(begin) = start.take() { process_xtoys_object(&message[begin..=idx], server, connection, patterns).await; } } } }
            _ => {}
        }
    }
}

fn process_command(
    command: &String<MAX_COMMAND_LENGTH>,
    server: &Server<'_>,
    patterns: &'static PatternSender,
) {
    info!("BLE Command {}", command);

    let mut split_command = command.split(":");

    let mut fail = false;

    if let Some(cmd) = split_command.next() {
        if let Some(action) = split_command.next() {
            match cmd {
                "set" => {
                    if let Some(value) = split_command.next() {
                        if let Ok(value) = value.parse::<u32>() {
                            let normalized = value as f64 / 100.0;
                            match action {
                                "speed" => patterns.set_speed(normalized),
                                "stroke" => patterns.set_stroke(normalized),
                                "depth" => patterns.set_depth(normalized),
                                // BLE sends 0–100; internal range is -1.0..1.0.
                                "sensation" => patterns.set_sensation(normalized * 2.0 - 1.0),
                                "pattern" => patterns.play(value as usize),
                                _ => {
                                    error!("Invalid set command {}", action);
                                    fail = true;
                                }
                            }
                        } else {
                            error!("Could not parse set value");
                            fail = true;
                        };
                    } else {
                        error!("No value after set");
                        fail = true;
                    }
                }
                "go" => match action {
                    "simplePenetration" | "strokeEngine" => patterns.play(0),
                    "pause" => patterns.pause(),
                    "resume" => patterns.resume(),
                    "menu" => patterns.stop(),
                    _ => {
                        error!("Unknown go action: {}", action);
                        fail = true;
                    }
                },
                _ => {
                    error!("Unknown command: {}", cmd);
                    fail = true;
                }
            }
        } else {
            error!("No action in command");
            fail = true;
        }
    } else {
        error!("Invalid command");
        fail = true;
    }

    let mut response_str: String<MAX_COMMAND_LENGTH> = String::new();
    if fail {
        response_str.write_str("fail:").expect("Should always fit");
        if response_str.write_str(command.as_str()).is_err() {
            response_str
                .write_str("overflow")
                .expect("Should always fit");
        }
    } else {
        response_str.write_str("ok:").expect("Should always fit");
        if response_str.write_str(command.as_str()).is_err() {
            response_str
                .write_str("overflow")
                .expect("Should always fit");
        }
    }
    if let Err(err) = server.set(&server.ossm_service.primary_command, &response_str) {
        error!("Failed to write the response to a set command {:?}", err);
    }
}

pub fn is_ble_connected() -> bool {
    CONNECTED.load(Ordering::Acquire)
}
