## OSSM-RS Simulator

Simulator that can runs the same motion control code as the machine
and displays graphs for position, velocity, and acceleration.

Can be used for developing patterns, testing or to just mess around.


## Running

### Native

```bash
cargo run --release
```

### Web

You may need to first run:

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

#### WASM Dev

```bash
trunk serve
```

#### WASM Release Build

```bash
trunk build --release
```

The built files will be in the `dist` directory
