use core::sync::atomic::Ordering;

use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_hal::rng::Rng;
use log::{info, warn};
use portable_atomic::{AtomicU64, AtomicU8};

use crate::wifi_settings::WifiFlash;

const SETTINGS_OFFSET: u32 = 0xB000;
const SETTINGS_END: u32 = 0xC000;
const RECORD_SIZE: usize = 64;
const MAGIC: &[u8; 8] = b"OSSAUTH1";
const VERSION: u8 = 1;
const MIN_PASSWORD_LEN: usize = 4;
const MAX_PASSWORD_LEN: usize = 64;
const HASH_ROUNDS: usize = 4096;

static CONFIGURED: AtomicU8 = AtomicU8::new(0);

fn checksum(data: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

fn password_hash(password: &str, salt: u64) -> u64 {
    // Deliberately dependency-free, salted and iterated for the ESP32-S3.
    // This is intended to prevent casual/local unauthorized configuration,
    // not to replace TLS or a memory-hard password KDF.
    let mut h = 0xcbf29ce484222325u64 ^ salt;
    for round in 0..HASH_ROUNDS {
        h ^= (round as u64).wrapping_mul(0x9e3779b97f4a7c15);
        for &b in password.as_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
            h ^= salt.rotate_left((b & 31) as u32);
        }
        h ^= h >> 32;
        h = h.wrapping_mul(0x9e3779b185ebca87);
    }
    h
}

pub async fn load(flash: &'static WifiFlash) -> bool {
    let mut record = [0u8; RECORD_SIZE];
    let mut guard = flash.lock().await;
    if guard.read(SETTINGS_OFFSET, &mut record).is_err() {
        warn!("Could not read owner authentication settings");
        CONFIGURED.store(0, Ordering::Release);
        return false;
    }
    drop(guard);

    if &record[..8] != MAGIC || record[8] != VERSION {
        CONFIGURED.store(0, Ordering::Release);
        return false;
    }
    let stored = u32::from_le_bytes(record[25..29].try_into().unwrap());
    if checksum(&record[..25]) != stored {
        warn!("Owner authentication checksum failed");
        CONFIGURED.store(0, Ordering::Release);
        return false;
    }
    CONFIGURED.store(1, Ordering::Release);
    info!("Owner page authentication enabled");
    true
}

pub fn configured() -> bool {
    CONFIGURED.load(Ordering::Acquire) != 0
}

pub async fn save_password(flash: &'static WifiFlash, password: &str) -> Result<(), ()> {
    if password.len() < MIN_PASSWORD_LEN || password.len() > MAX_PASSWORD_LEN {
        return Err(());
    }

    let rng = Rng::new();
    let salt = ((rng.random() as u64) << 32) | rng.random() as u64;
    let hash = password_hash(password, salt);

    let mut record = [0xffu8; RECORD_SIZE];
    record[..8].copy_from_slice(MAGIC);
    record[8] = VERSION;
    record[9..17].copy_from_slice(&salt.to_le_bytes());
    record[17..25].copy_from_slice(&hash.to_le_bytes());
    let sum = checksum(&record[..25]).to_le_bytes();
    record[25..29].copy_from_slice(&sum);

    let mut guard = flash.lock().await;
    guard.erase(SETTINGS_OFFSET, SETTINGS_END).map_err(|_| ())?;
    guard.write(SETTINGS_OFFSET, &record).map_err(|_| ())?;
    CONFIGURED.store(1, Ordering::Release);
    info!("Owner page password saved");
    Ok(())
}

pub async fn verify(flash: &'static WifiFlash, password: &str) -> bool {
    if !configured() {
        return false;
    }
    let mut record = [0u8; RECORD_SIZE];
    let mut guard = flash.lock().await;
    if guard.read(SETTINGS_OFFSET, &mut record).is_err() {
        return false;
    }
    drop(guard);
    if &record[..8] != MAGIC || record[8] != VERSION {
        return false;
    }
    let salt = u64::from_le_bytes(record[9..17].try_into().unwrap());
    let expected = u64::from_le_bytes(record[17..25].try_into().unwrap());
    password_hash(password, salt) == expected
}
