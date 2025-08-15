use std::path::Path;

fn main() {
    let dist = Path::new("webui/dist");
    if !dist.exists() {
        panic!("webui/dist missing. Run `npm ci && npm run build` in plugins/family_chat/webui before building the plugin.");
    }
}
