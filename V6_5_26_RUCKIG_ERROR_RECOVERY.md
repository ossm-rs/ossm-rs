# v6.5.26 XToys Ruckig diagnostic recovery

Based on v6.5.25.

Changes only the ordinary Ruckig error path:
- Logs the exact Ruckig update error and key input state.
- Signals the pending move as Cancelled instead of silently returning forever.
- Returns an ordinary failed Moving trajectory to Ready so XToys streaming can consume later queued moves.
- SexCode 10 ms direct-servo path is unchanged and does not use Ruckig.
- Serial diagnostics remain enabled; SexCode USB remains disabled in this diagnostic build.
