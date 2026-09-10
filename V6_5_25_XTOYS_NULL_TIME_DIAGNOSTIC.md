# v6.5.25 XToys Position-mode null-time diagnostic

Based on v6.5.24.

Changes:
- XToys JSON `move` packets with `"time":null` are treated as Position-mode initialization and are not queued as motion.
- Numeric `time:0` remains valid; retract/extend behavior is unchanged.
- Human-readable serial logging restored on COM8 for this diagnostic build.
- SexCode USB task disabled only for this diagnostic build to avoid corrupting/interleaving COM8 logs.
- Owner authentication, Wi-Fi recovery, persisted owner limits, and v6.5.24 XToys stop handoff are retained.
- Sexync 10 ms motion implementation source is otherwise unchanged, but SexCode USB transport is unavailable in this diagnostic build.
