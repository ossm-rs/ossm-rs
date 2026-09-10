# v6.5.44 clean core package

This package returns the project to the maintained XToys + owner-control core.

Retained unchanged in function:
- XToys BLE control, Position mode, Speed mode, pause/resume/stop behavior and firmware-owned routine support.
- Owner Wi-Fi page controls, persisted owner limits, machine settings, E-stop, BPM timed run, Wi-Fi setup and authentication.
- Owner USB RX control.
- Existing 10 ms OSSM motion baseline and XToys-required immediate hold behavior.

Removed/parked experimental integrations:
- Dedicated external-session protocol state and USB transport.
- PC Host/Guest web-host crate and command path.
- Public browser-room experiment (not part of this firmware workspace).
- Third-party Launch-emulation experiment.

The low-level direct-servo support in the OSSM motion controller remains because XToys uses it for immediate pause/stop hold semantics.
