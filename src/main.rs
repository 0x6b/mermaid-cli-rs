//! mermaid-cli-rs
//!
//! Convert Mermaid diagram to PNG or SVG format, without external network access.
use std::{
    error::Error,
    fs::{read, write},
    io::{stdin, Read},
    net::SocketAddr,
    sync::{Arc, RwLock},
};

use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::get,
    serve, Router,
};
use camino::Utf8PathBuf;
use clap::Parser;
use headless_chrome::{
    protocol::cdp::Page::CaptureScreenshotFormatOption::Png, Browser, LaunchOptionsBuilder,
};
use mime::{
    APPLICATION_JSON, FONT_WOFF, TEXT_CSS_UTF_8, TEXT_HTML, TEXT_JAVASCRIPT, TEXT_PLAIN_UTF_8,
};
use tokio::{net::TcpListener, spawn};

use crate::{
    macros::response,
    types::{Args, ImageFormat, SharedState, Store},
};

mod macros;
mod types;

/// HTML used to export a diagram to supported image
const HTML: &[u8] = include_bytes!("../assets/index.html");
/// Default font for the diagram
const FONT: &[u8] = include_bytes!("../assets/SourceHanSansJP-VF.otf.woff2");
/// Default stylesheet
const STYLE: &[u8] = include_bytes!("../assets/style.css");
/// Default configuration for Mermaid.js
const CONFIG: &[u8] = include_bytes!("../assets/config.json");
/// Mermaid.js bundle
const MERMAID_JS: &[u8] = include_bytes!("../assets/mermaid@10.6.1.min.mjs");

#[tokio::main(worker_threads = 2)]
async fn main() -> Result<(), Box<dyn Error>> {
    let Args {
        font,
        style,
        config,
        diagram,
        width,
        height,
        output,
    } = Args::parse();

    // A shared storage for resources used to serve.
    let shared_store = Arc::new(RwLock::new(Store {
        font: from_file_or_default(&font, FONT),
        style: from_file_or_default(&style, STYLE),
        config: from_file_or_default(&config, CONFIG),
        diagram: {
            if &diagram == "-" {
                let mut input = String::new();
                let mut handle = stdin().lock();
                handle.read_to_string(&mut input)?;
                input.into_bytes()
            } else {
                read(&diagram).unwrap_or_else(|_| panic!("Failed to read input file {}", diagram))
            }
        },
        mermaid_js: MERMAID_JS.to_vec(),
    }));

    // Create a server to handle HTTP requests.
    let app = Router::new()
        .route("/", get(|| async { response!(TEXT_HTML, HTML) }))
        .route(
            "/:path",
            get(|Path(path): Path<String>, State(state): State<SharedState>| async move {
                match state.read() {
                    Ok(store) => match path.as_ref() {
                        "font" => response!(FONT_WOFF, store.font),
                        "style" => response!(TEXT_CSS_UTF_8, store.style),
                        "config" => response!(APPLICATION_JSON, store.config),
                        "diagram" => response!(TEXT_PLAIN_UTF_8, store.diagram),
                        "mermaid_js" => response!(TEXT_JAVASCRIPT, store.mermaid_js),
                        _ => StatusCode::NOT_FOUND.into_response(),
                    },
                    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                }
            }),
        )
        .with_state(Arc::clone(&shared_store));

    // Bind the HTTP server to a local address.
    let addr = SocketAddr::from(([127, 0, 0, 1], 0));
    let listener = TcpListener::bind(&addr).await?;
    let port = listener.local_addr()?.port();
    spawn(async {
        serve(listener, app.into_make_service()).await.unwrap();
    });

    match export_mermaid_to_image(&output, width, height, port) {
        Ok(path) => println!("{path}"),
        Err(why) => panic!("{}", why.to_string()),
    }

    Ok(())
}

/// Export a Mermaid diagram to a file specified by the `output` argument.
///
/// # Arguments
///
/// * `output` - The path to the output file. The file format will be determined by the file
///   extension (e.g., `.png` for a PNG image, `.svg` for an SVG image).
/// * `width` - The width of the generated image.
/// * `height` - The height of the generated image.
/// * `port` - The port number to use for the local server serving the Mermaid diagram.
///
/// # Returns
///
/// A string representation of the path to the output file if the export was successful, or an error
/// if the export failed.
fn export_mermaid_to_image(
    output: &str,
    width: u32,
    height: u32,
    port: u16,
) -> Result<String, Box<dyn Error>> {
    let path = Utf8PathBuf::from(output);
    let image = convert_mermaid_to_image(width, height, ImageFormat::from(&path), port)?;
    write(&path, image)?;
    Ok(path.canonicalize()?.to_string_lossy().to_string())
}

/// Convert a Mermaid diagram to an image in the specified file format.
///
/// # Arguments
///
/// * `width` - The width of the generated image.
/// * `height` - The height of the generated image.
/// * `file_type` - The file format to use for the output image.
/// * `port` - The port number to use for the local server serving the Mermaid diagram.
///
/// # Returns
///
/// A `Result` containing `Vec<u8>` representing the generated image if the export was successful,
/// or an error if the export failed.
fn convert_mermaid_to_image(
    width: u32,
    height: u32,
    format: ImageFormat,
    port: u16,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let browser = Browser::new(
        LaunchOptionsBuilder::default()
            .window_size(Some((width, height)))
            .headless(true)
            .build()?,
    )?;
    let tab = browser.new_tab()?;

    tab.navigate_to(&format!("http://127.0.0.1:{port}/"))?;
    tab.wait_until_navigated()?;

    Ok(match format {
        ImageFormat::Svg => {
            let str = tab
                .wait_for_element("div#mermaid")?
                .call_js_fn("function() { return this.innerHTML; }", vec![], true)?
                .value
                .ok_or("failed to extract SVG")?
                .to_string()
                .replace(r#"\""#, r#"""#); // `this.innerHTML` returns double quoted string
            str[1..(str.len() - 1)].as_bytes().to_vec() // omit first and last "
        }
        ImageFormat::Png => tab
            .wait_for_element("div#mermaid > svg#svg")?
            .capture_screenshot(Png)?,
    })
}

/// Read a file from the given path or return a default value if the path is `None` or the file
/// cannot be read.
///
/// # Arguments
///
/// * `path` - An optional string representing the path to a file to read.
/// * `default` - A byte slice representing the default value to return if `path` is `None` or the
///   file cannot be read.
///
/// # Returns
///
/// A vector of bytes representing the contents of the file at the given path, or the default value
/// if the path is `None` or the file cannot be read.
fn from_file_or_default(path: &Option<String>, default: &[u8]) -> Vec<u8> {
    path.as_ref()
        .map_or_else(|| default.to_vec(), |path| read(path).unwrap_or_else(|_| default.to_vec()))
}
