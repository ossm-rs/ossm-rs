# v6.5.34 XToys pause/resume fix

- Adds explicit XToys JSON `pause` and `resume` handlers.
- Adds legacy text `go:pause` and `go:resume` handlers.
- Position streaming Pause no longer disables the motor or drops to Idle.
- Adds an internal `StreamingPaused` state that holds current position with the servo enabled.
- Resume returns directly to XToys Position streaming without reconnect/re-home.
- While Position streaming is paused, only the newest streamed target is retained to avoid replaying stale queued moves.
- Normal Stop still disables/returns Idle where appropriate.
- Retains v6.5.33 owner machine range and all prior XToys fixes.
