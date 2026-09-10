#![no_std]

pub mod commands;
mod engine;
mod input;
mod observer;
pub mod owner_limits;
mod pattern;
pub mod patterns;
mod runner;
mod sender;
mod util;

pub use engine::{EngineState, PatternEngine};
pub use input::{PatternInput, SharedPatternInput};
pub use observer::PatternObserver;
pub use pattern::{Pattern, PatternCtx};
pub use runner::PatternRunner;
pub use sender::PatternSender;
pub use util::scale;

/// Public-facing metadata for a pattern.
#[derive(Debug, Clone, Copy)]
pub struct PatternMeta {
    pub name: &'static str,
    pub description: &'static str,
}

use embedded_hal_async::delay::DelayNs;

use crate::patterns::{
    Deeper,
    HalfHalf,
    Hammerjack,
    Insist,
    Jackhammer,
    Knot,
    NonePattern,
    RoboStroke,
    Pull,
    Push,
    Simple,
    StopNGo,
    Stutter,
    TeasingPounding,
    Torque
};

// To add a new pattern:
// 1. Create a struct in `patterns/` that implements `Pattern`.
// 2. Re-export it from `patterns/mod.rs`.
// 3. Add a `Variant(Type)` entry below.
//
// The macro generates the `AnyPattern` enum, `Pattern` trait delegation,
// `From` impls, `BUILTIN_PATTERNS`, and `all_builtin()`.
// See `any_pattern_macro.rs` for the full definition.
include!("any_pattern_macro.rs");

define_patterns! {
    Simple(Simple),
    Deeper(Deeper),
    HalfHalf(HalfHalf),
    Hammerjack(Hammerjack),
    Insist(Insist),
    Jackhammer(Jackhammer),
    Knot(Knot),
    Pull(Pull),
    Push(Push),
    RoboStroke(RoboStroke),
    StopNGo(StopNGo),
    Stutter(Stutter),
    TeasingPounding(TeasingPounding),
    Torque(Torque),
    None(NonePattern),
}
