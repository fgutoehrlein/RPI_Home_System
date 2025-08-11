# Family Chat Plugin

This is a lightweight placeholder implementation of a local-only chat plugin for the homecore project. It exposes a tiny HTTP server and demonstrates the basic plugin lifecycle used by the core.

## Building

```
cargo build -p family_chat
```

## Running

The plugin is normally launched by the core. For manual testing you can run it directly:

```
# start HTTP server only
cargo run -p family_chat -- --bind 127.0.0.1:8787

# or run with the core stdio protocol
cargo run -p family_chat -- --stdio
```

Once running, open <http://localhost:8787> in a browser to view the placeholder UI.
