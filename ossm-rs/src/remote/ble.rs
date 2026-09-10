use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::config::{MAX_COMMAND_LENGTH, MAX_PATTERN_LENGTH, MAX_STATE_LENGTH};
use log::{error, info};
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker, Timer};
use esp_radio::ble::controller::BleConnector;
use heapless::String;
use trouble_host::prelude::*;

use ossm_motion::{
    motion::motion_state::{
        get_motion_state, set_motion_depth_pct, set_motion_enabled, set_motion_length_pct,
        set_motion_pattern, set_motion_sensation_pct, set_motion_velocity_pct,
    },
    pattern::PatternExecutor,
};

const SERVICE_UUID: Uuid = uuid!("522b443a-4f53-534d-0001-420badbabe69");
const PRIMARY_COMMAND_UUID: Uuid = uuid!("522b443a-4f53-534d-1000-420badbabe69");
const SPEED_KNOB_UUID: Uuid = uuid!("522b443a-4f53-534d-1010-420badbabe69");
const CURRENT_STATE_UUID: Uuid = uuid!("522b443a-4f53-534d-2000-420badbabe69");
const PATTERN_LIST_UUID: Uuid = uuid!("522b443a-4f53-534d-3000-420badbabe69");
const PATTERN_DESCRIPTION_UUID: Uuid = uuid!("522b443a-4f53-534d-3010-420badbabe69");

static CONNECTED: AtomicBool = AtomicBool::new(false);

#[gatt_server]
struct Server {
    ossm_service: OssmService,
}

#[gatt_service(uuid = SERVICE_UUID)]
struct OssmService {
    #[characteristic(uuid = PRIMARY_COMMAND_UUID, read, write)]
    primary_command: String<MAX_COMMAND_LENGTH>,

    #[characteristic(uuid = SPEED_KNOB_UUID, read, write)]
    speed_knob_characteristic: String<16>,

    #[characteristic(uuid = CURRENT_STATE_UUID, read, notify)]
    current_state: String<MAX_STATE_LENGTH>,

    #[characteristic(uuid = PATTERN_LIST_UUID, read)]
    pattern_list: String<MAX_PATTERN_LENGTH>,

    #[characteristic(uuid = PATTERN_DESCRIPTION_UUID, read, write)]
    pattern_description: String<MAX_PATTERN_LENGTH>,
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
) {
    info!("Starting advertising and GATT service");
    let server = Server::new_with_config(GapConfig::Peripheral(PeripheralConfig {
        name: "OSSM",
        appearance: &appearance::motorized_device::GENERIC_MOTORIZED_DEVICE,
    }))
    .unwrap();

    loop {
        match advertise("OSSM", &mut peripheral).await {
            Ok(connection) => {
                Timer::after_millis(100).await;

                // Try to set 2M PHY for better performance, but fall back to 1M if unsupported
                // (e.g., Bluetooth 4.2 devices like ESP32 don't support LE 2M PHY)
                if let Err(e) = connection.set_phy(stack, PhyKind::Le2M).await {
                    info!("Could not set 2M PHY ({:?}), continuing with 1M PHY", e);
                }

                let connect_params = ConnectParams {
                    min_connection_interval: Duration::from_micros(7500),
                    max_connection_interval: Duration::from_micros(7500),
                    ..Default::default()
                };
                connection
                    .update_connection_params(stack, &connect_params)
                    .await
                    .expect("Failed to update connection params");

                Timer::after_millis(100).await;

                let phy = connection.read_phy(stack).await.unwrap();
                let mtu = connection.att_mtu();
                info!("PHY {:?} MTU {:?}", phy, mtu);

                let gatt_connection = connection
                    .with_attribute_server(&server)
                    .expect("Could not transform connection into GATT connection");

                let events = gatt_events_task(&server, &gatt_connection);
                let notify = state_notifications(&server, &gatt_connection);

                match select(events, notify).await {
                    Either::First(res) => {
                        if let Err(err) = res {
                            panic!("[gatt] error in events task: {:?}", err);
                        }
                    }
                    Either::Second(res) => {
                        if let Err(err) = res {
                            panic!("[gatt] error in notify task: {:?}", err);
                        }
                    }
                }
            }
            Err(err) => {
                panic!("[adv] error: {:?}", err);
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
            panic!("[ble_task] error: {:?}", err);
        }
    }
}

async fn gatt_events_task<P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let reason = loop {
        match connection.next().await {
            GattConnectionEvent::Disconnected { reason } => break reason,
            GattConnectionEvent::Gatt { event } => {
                let mut write = false;
                let mut event_handle = 0;
                match &event {
                    GattEvent::Read(event) => {
                        if event.handle() == server.ossm_service.current_state.handle {
                            let state: String<MAX_STATE_LENGTH> = get_motion_state().as_json();
                            server.set(&server.ossm_service.current_state, &state)?;
                        }
                        if event.handle() == server.ossm_service.pattern_list.handle {
                            let patterns = PatternExecutor::new().get_all_patterns_json();
                            server.set(&server.ossm_service.pattern_list, &patterns)?;
                        }
                    }
                    GattEvent::Write(event) => {
                        write = true;
                        event_handle = event.handle();
                    }
                    _ => {}
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

                        process_command(&command, server);
                    }
                    if event_handle == server.ossm_service.pattern_description.handle {
                        let command: String<MAX_PATTERN_LENGTH> =
                            server.get(&server.ossm_service.pattern_description)?;

                        let description = if let Ok(index) = command.parse::<usize>() {
                            PatternExecutor::new().get_pattern_description(index)
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
            _ => {} // ignore other Gatt Connection Events
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
    let uuid: [u8; 16] = SERVICE_UUID
        .as_raw()
        .try_into()
        .expect("Service UUID incorrect");

    let mut advertiser_data = [0; 31];
    let len = AdStructure::encode_slice(
        &[
            AdStructure::Flags(LE_GENERAL_DISCOVERABLE | BR_EDR_NOT_SUPPORTED),
            AdStructure::ServiceUuids128(&[uuid]),
            AdStructure::CompleteLocalName(name.as_bytes()),
        ],
        &mut advertiser_data[..],
    )?;
    let advertiser = peripheral
        .advertise(
            &Default::default(),
            Advertisement::ConnectableScannableUndirected {
                adv_data: &advertiser_data[..len],
                scan_data: &[],
            },
        )
        .await?;
    info!("[adv] advertising");
    let conn = advertiser.accept().await?;
    CONNECTED.store(true, Ordering::Release);
    info!("[adv] connection established");
    Ok(conn)
}

async fn state_notifications<P: PacketPool>(
    server: &Server<'_>,
    connection: &GattConnection<'_, '_, P>,
) -> Result<(), Error> {
    let mut ticker = Ticker::every(Duration::from_millis(500));
    loop {
        let state: String<MAX_STATE_LENGTH> = get_motion_state().as_json();
        server
            .ossm_service
            .current_state
            .notify(connection, &state)
            .await?;
        ticker.next().await;
    }
}

fn process_command(command: &String<MAX_COMMAND_LENGTH>, server: &Server<'_>) {
    info!("BLE Command {}", command);

    let mut split_command = command.split(":");

    let mut fail = false;

    if let Some(cmd) = split_command.next() {
        if let Some(action) = split_command.next() {
            match cmd {
                "set" => {
                    if let Some(value) = split_command.next() {
                        if let Ok(value) = value.parse::<u32>() {
                            match action {
                                "speed" => {
                                    set_motion_velocity_pct(value);
                                }
                                "stroke" => {
                                    set_motion_length_pct(value);
                                }
                                "depth" => {
                                    set_motion_depth_pct(value);
                                }
                                "sensation" => {
                                    set_motion_sensation_pct(value);
                                }
                                "pattern" => {
                                    set_motion_pattern(value);
                                }
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
                    "simplePenetration" => {
                        set_motion_enabled(true);
                    }
                    "strokeEngine" => {
                        set_motion_enabled(true);
                    }
                    "menu" => {
                        set_motion_enabled(false);
                    }
                    _ => {
                        error!("Invalid go command {}", action);
                        fail = true;
                    }
                },
                _ => {
                    error!("Command neither set nor go");
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
