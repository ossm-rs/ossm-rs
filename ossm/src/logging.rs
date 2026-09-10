use core::fmt::Write;
use core::sync::atomic::{AtomicPtr, Ordering};

use embassy_time::Instant;

/// Output function type: receives a fully formatted log line.
type WriteFn = fn(&str);

static WRITE_FN: AtomicPtr<()> = AtomicPtr::new(core::ptr::null_mut());

struct OssmLogger;

impl log::Log for OssmLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        !WRITE_FN.load(Ordering::Relaxed).is_null()
    }

    fn log(&self, record: &log::Record) {
        let write_fn = WRITE_FN.load(Ordering::Relaxed);
        if write_fn.is_null() {
            return;
        }
        let write_fn: WriteFn = unsafe { core::mem::transmute(write_fn) };

        let elapsed = Instant::now().as_millis();
        let hours = elapsed / 3_600_000;
        let minutes = (elapsed % 3_600_000) / 60_000;
        let seconds = (elapsed % 60_000) / 1_000;
        let millis = elapsed % 1_000;

        let mut buf = heapless::String::<256>::new();
        let _ = write!(
            buf,
            "[{:02}:{:02}:{:02}.{:03}] [{:<5}] [{}] {}",
            hours,
            minutes,
            seconds,
            millis,
            record.level(),
            record.target(),
            record.args(),
        );

        write_fn(buf.as_str());
    }

    fn flush(&self) {}
}

static LOGGER: OssmLogger = OssmLogger;

/// Initialize the OSSM logger.
///
/// `write` receives a fully formatted log line and is responsible for
/// outputting it (e.g. UART, USB, console). The line does **not** include
/// a trailing newline — the write function should add one if needed.
///
/// # Example
///
/// ```ignore
/// ossm::logging::init(log::LevelFilter::Info, |line| {
///     esp_println::println!("{}", line);
/// });
/// ```
pub fn init(max_level: log::LevelFilter, write: WriteFn) {
    WRITE_FN.store(write as *mut (), Ordering::Relaxed);
    log::set_logger(&LOGGER).ok();
    log::set_max_level(max_level);
}
