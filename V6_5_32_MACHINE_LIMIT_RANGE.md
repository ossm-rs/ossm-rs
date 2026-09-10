# v6.5.32 machine limit range update

- Owner page Machine max speed range: 1..900 mm/s.
- Owner page Machine max length range: 1..400 mm.
- Persisted and boot-time validation use the same ceilings.
- Existing/default machine values remain 600 mm/s and 180 mm unless the owner explicitly changes them.
- Low-level MotionLimits still receives the selected machine values at reboot.
