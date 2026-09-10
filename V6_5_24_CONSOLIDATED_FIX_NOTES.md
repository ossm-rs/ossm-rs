# v6.5.24 consolidated XToys / Wi-Fi / owner security fixes

Base: v6.5.23 XToys/SexCode isolation.

## 1. XToys Position-mode deadlock fix

The captured failure occurred when XToys entered streaming/Position mode, then later sent `stop` while a timed Ruckig move was still active. The runner retargeted the motion but then awaited the old completion future, which could never complete. BLE remained connected and continued receiving JSON, while the pattern runner stopped consuming later commands.

v6.5.24 makes XToys streaming stop non-blocking:
- hold the current commanded position immediately with the already-proven direct-servo `speed=0` hold mechanism;
- clear the XToys stream queue;
- return the pattern runner to `Ready` immediately;
- the next normal XToys/pattern command takes over through the existing direct-stream handoff logic.

The SexCode 10 ms direct-servo implementation in `ossm/src/motion.rs` is unchanged from v6.5.23/v6.5.22 behavior.

## 2. Persisted owner limits active after boot

When owner master is enabled, the saved owner speed/stroke/depth limits are now authoritative even if the owner webpage is not connected. A page heartbeat/session is no longer required for XToys to use the configured speed range.

The legacy 10 mm/s fallback values remain available for diagnostics but are no longer selected merely because the owner webpage heartbeat timed out.

## 3. Wi-Fi bad-credentials recovery

If saved station credentials cannot associate for about 12 seconds at boot:
- only the Wi-Fi credentials record is cleared;
- the controller reboots into `OSSM-Setup` recovery AP mode;
- the owner password record and owner limits remain intact;
- BLE/XToys is also started while setup/recovery AP mode is active.

This avoids a permanent lockout after entering an incorrect Wi-Fi password.

## 4. Owner-page authentication

A separate owner-auth record is stored in flash sector `0xB000..0xC000`.
- Wi-Fi settings remain in `0x9000..0xA000`.
- Owner limits remain in `0xA000..0xB000`.
- Owner password material is stored as a salted, iterated hash; plaintext is not stored.
- Protected owner-page/API requests use HTTP Basic authentication.
- Username may be `owner`; only the chosen password is checked.
- E-stop and E-stop reset remain intentionally accessible without authentication.

### Existing installations

If saved Wi-Fi exists but no owner password exists yet, the normal owner URL first presents a one-time password creation page. After creating it, reload the page and the browser will request Basic authentication.

### First-time Wi-Fi setup

The `OSSM-Setup` page asks for Wi-Fi SSID/password and an owner password. The owner password is saved before the new Wi-Fi settings.

### Recovery AP

If an owner password already exists, opening the recovery setup page causes the browser to request Basic authentication before Wi-Fi credentials can be replaced.

### Security scope

The owner page currently uses plain HTTP, not HTTPS. Authentication prevents casual/unauthorized writes by nearby or LAN clients that do not know the password, but HTTP Basic credentials are not encrypted in transit. Treat this as local-access protection, not TLS-grade confidentiality.

## Build note

No new Cargo dependency was added and the existing lockfiles/pins are preserved. This package was source-checked and structurally compared against v6.5.23, but it was not Xtensa/ESP32-S3 compiled in the packaging environment.
