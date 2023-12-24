use std::{error::Error, fs::File, io::copy, path::Path};

use reqwest::blocking::get;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs"); // run `touch build.rs && cargo build` to force download assets
    let assets_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");

    for resource in [
        ("https://github.com/adobe-fonts/source-han-sans/raw/release/Variable/WOFF2/OTF/Subset/SourceHanSansJP-VF.otf.woff2", "SourceHanSansJP-VF.otf.woff2"),
        ("https://cdn.jsdelivr.net/npm/mermaid@10.6.1/dist/mermaid.min.js", "mermaid@10.6.1.min.mjs"),
    ] {
        let blob = get(resource.0)?.bytes()?;
        let mut out = File::create(assets_root.join(resource.1))?;
        copy(&mut blob.as_ref(), &mut out)?;
    }

    Ok(())
}
