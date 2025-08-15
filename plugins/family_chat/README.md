# Family Chat Plugin

A real-time chat service for HomeCore. It exposes a JSON/HTTP API and a
WebSocket endpoint backed by an embedded SQLite database and ships a
React/Vite web UI that is embedded into the binary at compile time.

## Features

* Multiple rooms and direct messages
* Presence, typing indicators and read receipts
* File uploads stored under a configurable data directory
* Full text search over messages
* Runs standalone over HTTP or as a plugin via the HomeCore stdio protocol
* Built-in Swagger UI for API exploration at `/swagger`

## Configuration

Family Chat reads configuration from a file, environment variables and CLI flags
with the following precedence:

`CLI` > `ENV` > `config file` > built-in defaults.

The default config file location is `config/family_chat.toml`. Override with the
`--config` flag or the `FAMILY_CHAT_CONFIG` environment variable. A sample file
is provided at `config/family_chat.example.toml`.

Supported settings:

* `bootstrap.username` / `bootstrap.password` – on first run, create this admin
  account. **The default `admin`/`admin` credentials are for local development
  only.**
* `server.port` – port to bind the HTTP/WS server (default `8787`). Host may be
  overridden with `--bind` or the `BIND` env variable.
* `logging.enabled` – when `false`, only warnings and errors are logged.
* `DATA_DIR` – directory for the SQLite database and uploaded files
* `MAX_UPLOAD_MB` – maximum upload size in megabytes (default `5`)

Environment variables `FAMILY_CHAT_PORT` and `FAMILY_CHAT_LOGGING` may override
the port and logging settings respectively.

## Building

Before compiling the plugin you need the web UI assets under `webui/dist`.
The `dist` directory is not committed to git and must be generated locally. The build will fail with a clear error if the directory is missing.
Ensure you are using Node.js 18 or newer. Generate the assets with:

```
cd plugins/family_chat/webui
npm ci
npm test
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
API documentation is available at <http://localhost:8787/swagger>.

## Testing

Run the backend and frontend test suites:

```
cargo test -p family_chat
cd plugins/family_chat/webui && npm test
```

