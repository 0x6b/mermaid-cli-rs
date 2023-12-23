use std::sync::{Arc, RwLock};

use camino::Utf8PathBuf;
use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about, version)]
pub(crate) struct Args {
    /// Path to the Mermaid diagram file. Specify `-` for stdin.
    #[arg(short = 'i', long = "input")]
    pub(crate) diagram: String,

    /// Path to the output file. By default, the file format is PNG. Specify a `.svg` extension if
    /// you need an SVG file.
    #[arg(short, long)]
    pub(crate) output: String,

    /// Width of the output image in pixels.
    #[arg(short, long, default_value = "1960")]
    pub(crate) width: u32,

    /// Height of the output image in pixels. This value is automatically reduced to fit the image.
    #[arg(short = 'H', long, default_value = "2160")]
    pub(crate) height: u32,

    /// Path to a CSS file for the HTML page.
    #[arg(short = 'c', long = "cssFile")]
    pub(crate) style: Option<String>,

    /// Path to a JSON configuration file for Mermaid.
    #[arg(short = 'C', long = "configFile")]
    pub(crate) config: Option<String>,

    /// Path to a font file for Mermaid.
    #[arg(short, long)]
    pub(crate) font: Option<String>,
}

/// Resources used by the application.
#[derive(Default)]
pub(crate) struct Store {
    /// The font used by the HTML page.
    pub(crate) font: Vec<u8>,

    /// The CSS styles used by the HTML page.
    pub(crate) style: Vec<u8>,

    /// The Mermaid configuration data.
    pub(crate) config: Vec<u8>,

    /// The input Mermaid diagram.
    pub(crate) diagram: Vec<u8>,

    /// The Mermaid.js code used by the HTML page.
    pub(crate) mermaid_js: Vec<u8>,
}

/// A type alias for a shared state.
pub(crate) type SharedState = Arc<RwLock<Store>>;

/// An enum representing supported image file format.
#[derive(Debug)]
pub(crate) enum ImageFormat {
    /// PNG file type. Default.
    Png,
    /// SVG file type.
    Svg,
}

impl From<&Utf8PathBuf> for ImageFormat {
    /// Converts the file path's extension to an `ImageFormat` enum.
    ///
    /// # Arguments
    ///
    /// * `path` - A reference to a `Utf8PathBuf` struct representing a file path.
    ///
    /// # Returns
    ///
    /// An `ImageFormat` enum based on the extension of the given file path.
    fn from(path: &Utf8PathBuf) -> Self {
        match path.as_path().extension() {
            Some("svg") => ImageFormat::Svg,
            _ => ImageFormat::Png,
        }
    }
}
