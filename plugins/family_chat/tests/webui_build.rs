use std::process::Command;

#[test]
fn webui_assets_build() {
    let webui_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/webui");

    // Install dependencies
    let status = Command::new("npm")
        .args(["ci"])
        .current_dir(webui_dir)
        .status()
        .expect("failed to run npm ci");
    assert!(status.success(), "npm ci failed");

    // Build into a temporary directory so the repository files are untouched
    let tmp = tempfile::tempdir().unwrap();
    let out_dir = tmp.path();
    let status = Command::new("npm")
        .args(["run", "build", "--", "--outDir", out_dir.to_str().unwrap()])
        .current_dir(webui_dir)
        .status()
        .expect("failed to run npm build");
    assert!(status.success(), "npm build failed");

    assert!(out_dir.join("index.html").exists(), "no build output");
}

