pub type TimerDuration = fugit::Duration<u64, 1, 1_000_000>;
pub type TimerInstant = fugit::Instant<u64, 1, 1_000_000>;

pub use TimerDuration as Duration;
pub use TimerInstant as Instant;

pub trait Timer {
    fn now(&self) -> Instant;
}
