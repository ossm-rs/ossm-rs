# v6.5.29 XToys Position / Pawprint tag collision fix

Based on v6.5.28 enabled-handoff diagnostic.

Observed failure:
- ordinary XToys Position move `time=46 ms` was decoded as firmware-routine `Pawprint UP`.
- 45/46/47 ms had been globally reserved as Pawprint DOWN/UP/warm-up markers.
- ordinary Position mode naturally generates these durations, so streaming was incorrectly stopped and later moves were only queued while Ready.

Fix:
- Pawprint 45/46/47 ms markers are recognized only when the firmware-owned routine transport is already in `XTOYS_ROUTINE_CAPTURE_LOCKED` state.
- In ordinary XToys Position mode, 45/46/47 ms are normal motion durations.
- Existing routine settings capture remains gated by its ARM/LOCK state.
- XToys enabled-handoff fix from v6.5.28 retained.
- v6.5.24 auth/Wi-Fi/persisted owner limits retained.
- Sexync 10 ms servo tuning unchanged.
- Serial diagnostic logging remains enabled; SexCode USB remains disabled for this diagnostic branch.
