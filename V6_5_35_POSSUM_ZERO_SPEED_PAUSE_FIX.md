# v6.5.35 Possum zero-speed pause/resume fix

- Legacy BLE clients such as Possum pause by ramping `set:speed` to 0 and resume by ramping speed back above 0.
- Starting `go:strokeEngine` while speed is still 0 now waits for a positive speed before starting a Ruckig trajectory.
- During an active normal pattern, speed 0 now soft-pauses the low-level motion while keeping the pattern future alive.
- When speed becomes positive again, the low-level motion resumes and continues the same pending stroke.
- Prevents zero-speed Ruckig cancellation from terminating the pattern and requiring another `go:strokeEngine`.
- XToys Position/SexCode behavior is unchanged by this patch.
