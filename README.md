# HomeCore

Rust workspace for a modular home automation system. The `core` process hosts
services such as logging, timers and storage, and loads external plugins over a
JSON/stdio protocol defined in the `plugin_api` crate.

## Building

Build the entire workspace:

```
cargo build --workspace
```

To build the `family_chat` web UI assets used by the plugin:

```
cd plugins/family_chat/webui
npm ci
npm test
npm run build
cd ../../..
cargo build -p family_chat
```

## Running the core

Start the core and load plugins from the `plugins/` directory:

```
cargo run -p core -- run --plugins-dir ./plugins
```

List discovered plugins:

```
cargo run -p core -- plugin list --plugins-dir ./plugins
```

## Plugins

* `sample_plugin` – Demonstrates the plugin protocol by subscribing to
  `timer.tick`, logging a message every second and replying to `sample.ping`
  requests. Run it manually with:

  ```
  cargo run -p sample_plugin -- --stdio
  ```

* `family_chat` – A chat service with rooms, presence, file uploads and a
  React/Vite web UI embedded into the binary. See
  [`plugins/family_chat/README.md`](plugins/family_chat/README.md) for build and
  run instructions.

## Tests

Run all Rust tests:

```
cargo test --workspace
```

The `family_chat` web UI also provides a JavaScript test suite:

```
cd plugins/family_chat/webui
npm ci
npm test
```

## CI

Native:

```
cargo test --workspace && cargo build --release
```

Aarch64 (local):

```
cargo install cross && cross build --release --target aarch64-unknown-linux-gnu
```

