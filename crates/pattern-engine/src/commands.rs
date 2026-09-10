//! Public command vocabulary for the pattern engine.
//!
//! [`PatternCommand`] is the unified command type. Issue commands by
//! constructing a `PatternCommand` and handing it to
//! [`PatternSender::apply`](crate::PatternSender::apply), or call the
//! per-action methods on [`PatternSender`](crate::PatternSender)
//! directly for the same effect.

use crate::{AnyPattern, PatternMeta};

/// Unified pattern command.
///
/// Combines one-shot playback transitions (`Play`/`Pause`/`Resume`/
/// `Stop`/`Home`) with continuous input values (`SetSpeed`/`SetDepth`/
/// `SetStroke`/`SetSensation`).
#[derive(Debug, Clone, Copy)]
pub enum PatternCommand {
    Play(usize),
    Pause,
    Resume,
    Stop,
    Home,
    /// 0.0-1.0 (fraction of max velocity).
    SetSpeed(f64),
    /// 0.0-1.0 (fraction of depth).
    SetStroke(f64),
    /// 0.0-1.0 (fraction of machine range).
    SetDepth(f64),
    /// -1.0-1.0 (pattern-specific).
    SetSensation(f64),
}

/// Static list of built-in pattern metadata.
pub fn pattern_list() -> &'static [PatternMeta] {
    &AnyPattern::BUILTIN_PATTERNS
}

/// Pattern description by index, if present.
pub fn pattern_description(idx: usize) -> Option<&'static str> {
    AnyPattern::BUILTIN_PATTERNS
        .get(idx)
        .map(|m| m.description)
}
