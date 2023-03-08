use std::{fs::File, io, path::Path};

use reqwest::blocking::get;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build.rs"); // run `touch build.rs && cargo build` to force download assets
    let assets_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");

    for resource in [
        ("https://github.com/adobe-fonts/source-han-sans/raw/release/Variable/WOFF2/OTF/Subset/SourceHanSansJP-VF.otf.woff2", "SourceHanSansJP-VF.otf.woff2"),
        ("https://raw.githubusercontent.com/adobe-fonts/source-han-sans/master/LICENSE.txt", "SourceHanSansJP-VF.otf.woff2.LICENSE"),
        ("https://cdn.jsdelivr.net/npm/mermaid@9.4.0/dist/mermaid.min.js", "mermaid@9.4.0.min.js"),
        ("https://raw.githubusercontent.com/mermaid-js/mermaid/v9.4.0/LICENSE", "mermaid@9.4.0.min.js.LICENSE"),
    ] {
        let url = resource.0;
        let file = assets_root.join(resource.1);
        println!("cargo:warning=downloading {:?} from {}", &file, &url);

        let blob = get(url)?.bytes()?;
        let mut out = File::create(file)?;
        io::copy(&mut blob.as_ref(), &mut out)?;
    }

    Ok(())
}
