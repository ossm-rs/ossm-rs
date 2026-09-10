# OSSM-RS Firmware

The ossm-rs implementation for microcontrollers based on ESP32.

Receives commands from remotes using BLE or ESP-NOW and uses the `ossm_motion` crate under the hood for motion control and patterns.

See the [main README](../README.md) on how to build and upload

## Developing OSSM-RS Firmware

Open the entire workspace to build the firmware and make changes to xtask. Open the subdirectory `ossm-rs` to develop the firmware.

Edit the [config.toml](../ossm-rs/.cargo/config.toml#l15) in `ossm-rs` file with the appropriate build targets to better feedback from rust-analyzer.

If using VSCode add a .vscode/settings.json with the following json to enable specific features (and thus boards) of ossm-rs:

```json
{
  "rust-analyzer.cargo.features": "board_ossm_alt_v2"
}
```

For other editors add a default feature under `features` with the board that you are using to [Cargo.toml](../ossm-rs/Cargo.toml) in `ossm-rs`:

```toml
[features]
default = ["board_ossm_alt_v2"]
```

This split is because there are multiple build targets and feature flags needed to build both xtask and ossm-rs - something that rust-analyzer does not support.
