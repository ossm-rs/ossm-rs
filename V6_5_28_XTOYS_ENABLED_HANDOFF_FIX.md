# v6.5.28 XToys enabled handoff fix

Diagnostic/test build based on v6.5.26.

Fixes the confirmed XToys Speed -> Position handoff bug:
- XToys sends `stop` before `startStreaming`.
- In `RunnerState::Playing`, that `XToysStop` previously called `motion.disable()`.
- The pattern engine then published `Ready`, but the low-level motion controller was actually Disabled.
- Position moves and later Speed commands were therefore received but could not drive the motor.

v6.5.28 replaces that disable with an immediate hold at the current commanded position using the existing direct-stream speed=0 hold path. The motor remains enabled and homing remains valid. The first following normal XToys command automatically takes over.

Retained:
- v6.5.24 owner password / Wi-Fi recovery / saved owner limits
- v6.5.25 `time:null` Position seed handling
- v6.5.26 Ruckig error logging/recovery
- readable COM8 application logging; SexCode USB remains disabled in this diagnostic build
- Sexync 10 ms servo tuning unchanged
