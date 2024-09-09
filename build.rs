use std::{error::Error, fs::File, io::copy, path::Path};

use reqwest::blocking::get;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs"); // run `touch build.rs && cargo build` to force download assets
    let assets_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");

    let blob = get("https://cdn.jsdelivr.net/npm/mermaid@10.6.1/dist/mermaid.min.js")?.bytes()?;
    let mut out = File::create(assets_root.join("mermaid@10.6.1.min.mjs"))?;
    copy(&mut blob.as_ref(), &mut out)?;
    Ok(())
}
