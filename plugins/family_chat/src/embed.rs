use include_dir::{include_dir, Dir};

// Embedded static files for the web UI.
pub static WEB_DIST: Dir = include_dir!("$CARGO_MANIFEST_DIR/webui/dist");
