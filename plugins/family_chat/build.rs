use std::path::Path;
fn main() {
    let dist = Path::new("webui/dist");
    if !dist.exists() {
        panic!("webui/dist not found. Run `npm ci && npm run build` in webui/ first.");
    }
    println!("cargo:rerun-if-changed=webui/dist");
}
