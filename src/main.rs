//! mermaid-cli-rs
//!
//! Convert Mermaid diagram to PNG or SVG format, without external network access.

use anyhow::Result;
use clap::Parser;

use crate::{exporter::Exporter, types::Args};

mod exporter;
mod macros;
mod types;

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<()> {
    let Args { style, config, diagram, width, height, output } = Args::parse();

    let exporter = Exporter::new(&diagram, style, config).await?;
    let exporter = exporter.launch().await?;
    exporter.export_mermaid_to_image(&output, width, height).await?;
    println!("{output}");

    Ok(())
}
