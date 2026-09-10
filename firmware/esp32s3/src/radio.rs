use core::net::Ipv4Addr;

use embassy_executor::Spawner;
use embassy_net::{
    Config as NetConfig, Ipv4Address, Ipv4Cidr, Runner as NetRunner, Stack, StackResources,
    StaticConfigV4,
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Timer};
use esp_hal::{
    peripherals::{BT, WIFI},
    rng::Rng,
};
use esp_radio::ble::controller::BleConnector;
use esp_radio::esp_now::{EspNowManager, EspNowSender};
use esp_radio::wifi::{AccessPointConfig, ClientConfig, ModeConfig, WifiController, WifiDevice, WifiEvent};
use log::{info, warn};
use ossm::MotionLimits;
use ossm_m5_remote::RemoteConfig;
use pattern_engine::PatternSender;

use crate::{
    mk_static,
    owner_web::{owner_web_task, wifi_setup_web_task},
    wifi_settings::{self, WifiFlash},
};

const SETUP_AP_SSID: &str = "OSSM-Setup";

#[embassy_executor::task]
async fn wifi_reconnect_task(mut controller: WifiController<'static>) {
    loop {
        controller.wait_for_event(WifiEvent::StaDisconnected).await;
        warn!("Owner Wi-Fi disconnected");
        loop {
            Timer::after(Duration::from_secs(2)).await;
            info!("Reconnecting owner Wi-Fi");
            match controller.connect_async().await {
                Ok(()) => {
                    info!("Owner Wi-Fi re-associated");
                    break;
                }
                Err(e) => warn!("Owner Wi-Fi reconnect failed: {:?}", e),
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: NetRunner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn setup_dhcp_task(stack: Stack<'static>) {
    let config = esp_hal_dhcp::structs::DhcpServerConfig {
        ip: Ipv4Addr::new(192, 168, 4, 1),
        lease_time: Duration::from_secs(3600),
        gateways: &[],
        subnet: None,
        dns: &[],
        use_captive_portal: true,
    };
    let mut leaser = esp_hal_dhcp::simple_leaser::SimpleDhcpLeaser {
        start: Ipv4Addr::new(192, 168, 4, 20),
        end: Ipv4Addr::new(192, 168, 4, 100),
        leases: Default::default(),
    };
    let _ = esp_hal_dhcp::run_dhcp_server(stack, config, &mut leaser).await;
}

pub async fn start(
    spawner: &Spawner,
    wifi: WIFI<'static>,
    bt: BT<'static>,
    patterns: &'static PatternSender,
    limits: &MotionLimits,
    wifi_flash: &'static WifiFlash,
) {
    let radio = &*mk_static!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    );
    let (mut wifi_controller, interfaces) =
        esp_radio::wifi::new(radio, wifi, Default::default()).unwrap();

    // Wi-Fi credentials are only loaded from flash. If none are stored, or the
    // user explicitly requested setup mode, boot the OSSM-Setup access point.
    // This keeps distributable source packages free of build-time credentials.
    let credentials = if wifi_settings::setup_requested(wifi_flash).await {
        info!("Wi-Fi setup mode requested");
        None
    } else {
        match wifi_settings::load(wifi_flash).await {
            Some(saved) => {
                info!("Using saved Wi-Fi settings for SSID '{}'", saved.ssid.as_str());
                Some(saved)
            }
            None => None,
        }
    };

    if credentials.is_none() {
        // First-time/recovery setup mode. ESP-NOW stays disabled; BLE remains
        // available so XToys control is not lost while Wi-Fi is repaired.
        let ap = ModeConfig::AccessPoint(
            AccessPointConfig::default()
                .with_ssid(SETUP_AP_SSID.into())
                .with_max_connections(4),
        );
        wifi_controller
            .set_config(&ap)
            .expect("Failed to configure OSSM setup access point");
        wifi_controller.start().unwrap();

        let seed = {
            let rng = Rng::new();
            ((rng.random() as u64) << 32) | rng.random() as u64
        };
        let net_config = NetConfig::ipv4_static(StaticConfigV4 {
            address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 4, 1), 24),
            gateway: None,
            dns_servers: Default::default(),
        });
        let (stack, runner) = embassy_net::new(
            interfaces.ap,
            net_config,
            mk_static!(StackResources<4>, StackResources::<4>::new()),
            seed,
        );

        spawner.must_spawn(net_task(runner));
        spawner.must_spawn(setup_dhcp_task(stack));
        spawner.must_spawn(wifi_setup_web_task(stack, wifi_flash));

        // BLE remains available during first-time/recovery setup so a bad Wi-Fi
        // password never removes XToys control. Wi-Fi AP + BLE coexist on the
        // same radio controller.
        let connector = BleConnector::new(radio, bt, Default::default())
            .expect("Could not create BleConnector in setup mode");
        ble_remote::start(spawner, connector, patterns);

        info!("Wi-Fi setup/recovery mode active; BLE also available");
        info!("Connect to '{}' then open http://192.168.4.1/", SETUP_AP_SSID);

        // Keep the Wi-Fi controller alive until the setup page saves settings
        // and reboots the chip.
        loop {
            Timer::after(Duration::from_secs(60)).await;
        }
    }

    let credentials = credentials.unwrap();
    let client = ModeConfig::Client(
        ClientConfig::default()
            .with_ssid(credentials.ssid.as_str().into())
            .with_password(credentials.password.as_str().into()),
    );
    wifi_controller
        .set_config(&client)
        .expect("Failed to configure owner Wi-Fi");
    wifi_controller.start().unwrap();

    let wifi_interface = interfaces.sta;
    let esp_now = interfaces.esp_now;

    let rng = Rng::new();
    let seed = ((rng.random() as u64) << 32) | rng.random() as u64;
    let (net_stack, net_runner) = embassy_net::new(
        wifi_interface,
        NetConfig::dhcpv4(Default::default()),
        mk_static!(StackResources<4>, StackResources::<4>::new()),
        seed,
    );

    info!("ESP-NOW version {}", esp_now.version().unwrap());
    let (manager, sender, receiver) = esp_now.split();
    let manager = mk_static!(EspNowManager<'static>, manager);
    let sender = mk_static!(
        Mutex::<NoopRawMutex, EspNowSender<'static>>,
        Mutex::<NoopRawMutex, _>::new(sender)
    );

    let remote_config = RemoteConfig {
        max_velocity_mm_s: limits.max_velocity_mm_s,
        max_travel_mm: limits.max_position_mm - limits.min_position_mm,
    };

    // Known-good baseline: LAN Wi-Fi + BLE on ProCpu, with M5/ESP-NOW disabled.
    info!("M5/ESP-NOW disabled while owner LAN Wi-Fi is active");
    let _ = (manager, sender, receiver, remote_config);

    info!("Owner radio mode: Wi-Fi -> BLE");
    spawner.must_spawn(net_task(net_runner));
    spawner.must_spawn(owner_web_task(net_stack, patterns, wifi_flash));

    let connect_started = Instant::now();
    loop {
        info!("Connecting owner Wi-Fi to SSID '{}'", credentials.ssid.as_str());
        match wifi_controller.connect_async().await {
            Ok(()) => {
                info!("Owner Wi-Fi associated; delaying BLE startup for DHCP/network settle");
                break;
            }
            Err(e) => {
                warn!("Owner Wi-Fi connection failed: {:?}", e);
                if Instant::now().duration_since(connect_started) >= Duration::from_secs(12) {
                    warn!("Saved Wi-Fi failed for 12s; clearing Wi-Fi record and rebooting to recovery AP");
                    if wifi_settings::clear(wifi_flash).await.is_err() {
                        warn!("Could not clear failed Wi-Fi settings; retrying");
                    } else {
                        Timer::after(Duration::from_millis(250)).await;
                        esp_hal::system::software_reset();
                    }
                }
                Timer::after(Duration::from_secs(2)).await;
            }
        }
    }

    Timer::after(Duration::from_secs(4)).await;

    let connector = BleConnector::new(radio, bt, Default::default())
        .expect("Could not create BleConnector");
    ble_remote::start(spawner, connector, patterns);
    info!("BLE started; Wi-Fi remains associated");

    spawner.must_spawn(wifi_reconnect_task(wifi_controller));
}
