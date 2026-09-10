use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_storage::FlashStorage;
use heapless::String;
use log::{info, warn};

pub type WifiFlash = Mutex<NoopRawMutex, FlashStorage<'static>>;

const SETTINGS_OFFSET: u32 = 0x9000;
const SETTINGS_END: u32 = 0xA000;
const RECORD_SIZE: usize = 128;
const MAGIC: &[u8; 8] = b"OSSWIFI1";
const VERSION_CREDENTIALS: u8 = 1;
const VERSION_SETUP_REQUESTED: u8 = 2;

#[derive(Clone)]
pub struct WifiCredentials {
    pub ssid: String<32>,
    pub password: String<64>,
}

impl WifiCredentials {
    pub fn new(ssid: &str, password: &str) -> Option<Self> {
        if ssid.is_empty() || ssid.len() > 32 || password.len() > 64 {
            return None;
        }
        let mut s = String::<32>::new();
        let mut p = String::<64>::new();
        s.push_str(ssid).ok()?;
        p.push_str(password).ok()?;
        Some(Self { ssid: s, password: p })
    }
}

fn checksum(data: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

pub async fn load(flash: &'static WifiFlash) -> Option<WifiCredentials> {
    let mut record = [0u8; RECORD_SIZE];
    let mut guard = flash.lock().await;
    if guard.read(SETTINGS_OFFSET, &mut record).is_err() {
        warn!("Could not read saved Wi-Fi settings");
        return None;
    }
    drop(guard);

    if &record[..8] != MAGIC || record[8] != VERSION_CREDENTIALS {
        return None;
    }
    let ssid_len = record[9] as usize;
    let password_len = record[10] as usize;
    if ssid_len == 0 || ssid_len > 32 || password_len > 64 {
        return None;
    }
    let data_end = 12 + ssid_len + password_len;
    if data_end + 4 > RECORD_SIZE {
        return None;
    }
    let stored = u32::from_le_bytes(record[data_end..data_end + 4].try_into().ok()?);
    if checksum(&record[..data_end]) != stored {
        warn!("Saved Wi-Fi settings checksum failed");
        return None;
    }
    let ssid = core::str::from_utf8(&record[12..12 + ssid_len]).ok()?;
    let password = core::str::from_utf8(&record[12 + ssid_len..data_end]).ok()?;
    WifiCredentials::new(ssid, password)
}

pub async fn save(flash: &'static WifiFlash, credentials: &WifiCredentials) -> Result<(), ()> {
    let mut record = [0xffu8; RECORD_SIZE];
    record[..8].copy_from_slice(MAGIC);
    record[8] = VERSION_CREDENTIALS;
    record[9] = credentials.ssid.len() as u8;
    record[10] = credentials.password.len() as u8;
    record[11] = 0;
    let mut pos = 12;
    record[pos..pos + credentials.ssid.len()].copy_from_slice(credentials.ssid.as_bytes());
    pos += credentials.ssid.len();
    record[pos..pos + credentials.password.len()].copy_from_slice(credentials.password.as_bytes());
    pos += credentials.password.len();
    let sum = checksum(&record[..pos]).to_le_bytes();
    record[pos..pos + 4].copy_from_slice(&sum);

    let mut guard = flash.lock().await;
    guard.erase(SETTINGS_OFFSET, SETTINGS_END).map_err(|_| ())?;
    guard.write(SETTINGS_OFFSET, &record).map_err(|_| ())?;
    info!("Wi-Fi settings saved for SSID '{}'", credentials.ssid.as_str());
    Ok(())
}

pub async fn setup_requested(flash: &'static WifiFlash) -> bool {
    let mut record = [0u8; 12];
    let mut guard = flash.lock().await;
    if guard.read(SETTINGS_OFFSET, &mut record).is_err() {
        return false;
    }
    &record[..8] == MAGIC && record[8] == VERSION_SETUP_REQUESTED
}

/// Clear saved credentials and explicitly request setup mode on next boot.
/// This marker prevents build-time/factory credentials from silently taking
/// over again after the user selected "Clear Wi-Fi" on the owner page.
pub async fn clear(flash: &'static WifiFlash) -> Result<(), ()> {
    let mut record = [0xffu8; RECORD_SIZE];
    record[..8].copy_from_slice(MAGIC);
    record[8] = VERSION_SETUP_REQUESTED;

    let mut guard = flash.lock().await;
    guard.erase(SETTINGS_OFFSET, SETTINGS_END).map_err(|_| ())?;
    guard.write(SETTINGS_OFFSET, &record).map_err(|_| ())?;
    info!("Saved Wi-Fi settings cleared; setup mode requested");
    Ok(())
}
