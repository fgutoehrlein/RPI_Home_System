# Family Chat Plugin

This plugin now ships a small React-based web UI that is embedded into the
Rust binary. The UI is built with Vite and TypeScript and compiled to static
assets under `webui/dist` which are embedded at build time.

## Building

Before compiling the plugin you need the web UI assets under `webui/dist`.
Generate them with:

```
cd plugins/family_chat/webui
npm ci
npm run build
```

After the assets are generated, build the Rust plugin as usual:

```
cargo build -p family_chat
```

## Running

The plugin is normally launched by the core. For manual testing you can run it
directly:

```
# start HTTP server only
cargo run -p family_chat -- --bind 127.0.0.1:8787

# or run with the core stdio protocol
cargo run -p family_chat -- --stdio
```

Once running, open <http://localhost:8787> in a browser to view the chat UI.
