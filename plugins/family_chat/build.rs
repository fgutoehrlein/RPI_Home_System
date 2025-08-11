use std::{path::Path, process::Command};

fn main() {
    let dist = Path::new("webui/dist");
    if !dist.exists() {
        // Missing build artefacts: attempt to build the web UI so CI
        // and first-time checkouts work without manual steps.
        let status = Command::new("npm")
            .args(["ci"])
            .current_dir("webui")
            .status()
            .expect("failed to run `npm ci`");
        if !status.success() {
            panic!("npm ci failed; ensure Node and npm are installed");
        }

        let status = Command::new("npm")
            .args(["run", "build"])
            .current_dir("webui")
            .status()
            .expect("failed to run `npm run build`");
        if !status.success() {
            panic!("npm run build failed");
        }
    }

    println!("cargo:rerun-if-changed=webui/package.json");
    println!("cargo:rerun-if-changed=webui/src");
    println!("cargo:rerun-if-changed=webui/index.html");
    println!("cargo:rerun-if-changed=webui/tailwind.config.cjs");
    println!("cargo:rerun-if-changed=webui/postcss.config.cjs");
    println!("cargo:rerun-if-changed=webui/tsconfig.json");
    println!("cargo:rerun-if-changed=webui/vite.config.ts");
    println!("cargo:rerun-if-changed=webui/public");
    println!("cargo:rerun-if-changed=webui/dist");
}
