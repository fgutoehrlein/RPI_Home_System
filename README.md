# HomeCore

Rust workspace demonstrating a simple home automation core that loads plugins which communicate over JSON and stdio.

## Build

```
cargo build
```

## Run

Start the core and load plugins from the `plugins/` directory:

```
cargo run -p core -- run --plugins-dir ./plugins
```

List discovered plugins:

```
cargo run -p core -- plugin list --plugins-dir ./plugins
```

## Sample plugin

The sample plugin subscribes to `timer.tick` and logs a message every second. It also handles a `sample.ping` request and echoes the payload.

## Tests

```
cargo test --workspace
```

## CI

Native:

```
cargo test --workspace && cargo build --release
```

Aarch64 (lokal):

```
cargo install cross && cross build --release --target aarch64-unknown-linux-gnu
```
