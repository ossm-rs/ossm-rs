use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use log::{info, warn};
use pattern_engine::owner_limits;

use crate::wifi_settings::WifiFlash;

const SETTINGS_OFFSET: u32 = 0xA000;
const SETTINGS_END: u32 = 0xB000;
const RECORD_SIZE: usize = 80;
const MAGIC: &[u8; 8] = b"OSSLIM01";
const VERSION_V1: u8 = 1;
const VERSION_V2: u8 = 2;

fn checksum(data: &[u8]) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}

fn put_f64(dst: &mut [u8], value: f64) {
    dst.copy_from_slice(&value.to_le_bytes());
}

fn get_f64(src: &[u8]) -> f64 {
    f64::from_le_bytes(src.try_into().unwrap())
}

/// Load persisted machine configuration and owner limits. Machine settings are
/// applied first so owner limits are clamped against the correct physical
/// envelope before the motion controller is constructed.
pub async fn load_and_apply(flash: &'static WifiFlash) -> bool {
    let mut record = [0u8; RECORD_SIZE];
    let mut guard = flash.lock().await;
    if guard.read(SETTINGS_OFFSET, &mut record).is_err() {
        warn!("Could not read saved owner limits");
        return false;
    }
    drop(guard);

    if &record[..8] != MAGIC {
        return false;
    }

    let version = record[8];
    if version == VERSION_V1 {
        let stored = u32::from_le_bytes(record[57..61].try_into().unwrap());
        if checksum(&record[..57]) != stored {
            warn!("Saved owner limits checksum failed");
            return false;
        }
    } else if version == VERSION_V2 {
        let stored = u32::from_le_bytes(record[72..76].try_into().unwrap());
        if checksum(&record[..72]) != stored {
            warn!("Saved owner/machine settings checksum failed");
            return false;
        }
    } else {
        return false;
    }

    let master_enabled = record[9] != 0;
    let max_speed = get_f64(&record[16..24]);
    let min_stroke = get_f64(&record[24..32]);
    let max_stroke = get_f64(&record[32..40]);
    let min_depth = get_f64(&record[40..48]);
    let max_depth = get_f64(&record[48..56]);
    if !max_speed.is_finite()
        || !min_stroke.is_finite()
        || !max_stroke.is_finite()
        || !min_depth.is_finite()
        || !max_depth.is_finite()
    {
        warn!("Saved owner limits contain invalid values");
        return false;
    }

    // v1 records predate configurable machine geometry. Keep the compiled
    // 600 mm/s / 180 mm defaults when migrating them; the configurable ceilings are higher.
    if version == VERSION_V2 {
        let machine_speed = get_f64(&record[56..64]);
        let machine_travel = get_f64(&record[64..72]);
        if !machine_speed.is_finite() || !machine_travel.is_finite()
            || machine_speed < 1.0 || machine_speed > owner_limits::HARD_MAX_MACHINE_SPEED_MM_S
            || machine_travel < 1.0 || machine_travel > owner_limits::HARD_MAX_MACHINE_TRAVEL_MM
        {
            warn!("Saved machine settings contain invalid values");
            return false;
        }
        owner_limits::configure_machine(machine_speed, machine_travel);
        info!("Restored machine limits from flash: speed={:.1} mm/s length={:.1} mm", machine_speed, machine_travel);
    }

    owner_limits::set_owner_limits(max_speed, min_stroke, max_stroke, min_depth, max_depth);
    owner_limits::set_master_enabled(master_enabled);
    let applied = owner_limits::owner_limits();
    info!(
        "Restored owner limits from flash: master={} speed={:.1} stroke={:.1}..{:.1} depth={:.1}..{:.1}",
        master_enabled, applied.max_speed, applied.min_stroke, applied.max_stroke, applied.min_depth, applied.max_depth
    );
    true
}

fn build_record(machine_speed: f64, machine_travel: f64) -> [u8; RECORD_SIZE] {
    let limits = owner_limits::owner_limits();
    let mut record = [0xffu8; RECORD_SIZE];
    record[..8].copy_from_slice(MAGIC);
    record[8] = VERSION_V2;
    record[9] = owner_limits::master_enabled() as u8;
    // Bytes 10..16 reserved for future settings.
    put_f64(&mut record[16..24], limits.max_speed.min(machine_speed));
    put_f64(&mut record[24..32], limits.min_stroke.min(machine_travel));
    put_f64(&mut record[32..40], limits.max_stroke.min(machine_travel));
    put_f64(&mut record[40..48], limits.min_depth.min(machine_travel));
    put_f64(&mut record[48..56], limits.max_depth.min(machine_travel));
    put_f64(&mut record[56..64], machine_speed);
    put_f64(&mut record[64..72], machine_travel);
    let sum = checksum(&record[..72]).to_le_bytes();
    record[72..76].copy_from_slice(&sum);
    record
}

async fn write_record(flash: &'static WifiFlash, record: &[u8; RECORD_SIZE]) -> Result<(), ()> {
    let mut guard = flash.lock().await;
    guard.erase(SETTINGS_OFFSET, SETTINGS_END).map_err(|_| ())?;
    guard.write(SETTINGS_OFFSET, record).map_err(|_| ())?;
    Ok(())
}

pub async fn save_current(flash: &'static WifiFlash) -> Result<(), ()> {
    let record = build_record(owner_limits::machine_max_speed_mm_s(), owner_limits::machine_travel_mm());
    write_record(flash, &record).await?;
    info!("Owner/machine settings saved to flash");
    Ok(())
}

/// Save a new machine envelope. It is deliberately not applied live: the
/// motion controller owns a boot-time MotionLimits copy, so applying only the
/// web-side scaling before reboot would create an unsafe mismatch.
pub async fn save_machine(flash: &'static WifiFlash, max_speed_mm_s: f64, travel_mm: f64) -> Result<(), ()> {
    if !max_speed_mm_s.is_finite() || !travel_mm.is_finite()
        || max_speed_mm_s < 1.0 || max_speed_mm_s > owner_limits::HARD_MAX_MACHINE_SPEED_MM_S
        || travel_mm < 1.0 || travel_mm > owner_limits::HARD_MAX_MACHINE_TRAVEL_MM
    {
        return Err(());
    }
    let record = build_record(max_speed_mm_s, travel_mm);
    write_record(flash, &record).await?;
    info!("New machine limits saved: speed={:.1} mm/s length={:.1} mm; reboot required", max_speed_mm_s, travel_mm);
    Ok(())
}
