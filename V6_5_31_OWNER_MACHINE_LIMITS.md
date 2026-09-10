# v6.5.31 - Owner-page machine max speed / length

Adds persistent machine-level configuration to the authenticated owner page:

- Machine max speed: 1..900 mm/s
- Machine max length: 1..400 mm
- Saving stops motion, persists the new machine envelope, then reboots.
- On boot the persisted values are applied to the actual low-level `MotionLimits` before the 10 ms motion controller is constructed.
- Existing v1 owner-limit flash records remain compatible and migrate using the compiled 600 mm/s / 180 mm machine defaults.
- Owner speed/stroke/depth limits are clamped against the configured machine envelope.
- XToys Speed/Position, BPM, firmware routines and Sexync all use the same configured machine speed/travel scaling via `owner_limits`.
- Configurable ceilings are 900 mm/s and 400 mm. Defaults remain 600 mm/s and 180 mm so upgrades do not silently expand an existing machine.

Based on v6.5.30; no Sexync 10 ms servo tuning changes. Serial diagnostics remain enabled.
