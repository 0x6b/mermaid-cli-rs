use std::{
    io::{stdin, Read},
    net::SocketAddr,
    ops::Deref,
    sync::{Arc, RwLock},
};

use anyhow::{anyhow, Result};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, IntoMakeService},
    serve, Router,
};
use camino::Utf8PathBuf;
use headless_chrome::{
    protocol::cdp::Page::CaptureScreenshotFormatOption::Png, Browser, LaunchOptionsBuilder,
};
use mime::{APPLICATION_JSON, TEXT_CSS_UTF_8, TEXT_HTML, TEXT_JAVASCRIPT, TEXT_PLAIN_UTF_8};
use tokio::{
    fs::{read, read_to_string, write},
    net::TcpListener,
    spawn,
};

use crate::{
    macros::response,
    types::{ImageFormat, SharedState, Store},
};

/// HTML used to export a diagram to supported image
const HTML: &[u8] = include_bytes!("../assets/index.html");
/// Default stylesheet
const STYLE: &[u8] = include_bytes!("../assets/style.css");
/// Default configuration for Mermaid.js
const CONFIG: &[u8] = include_bytes!("../assets/config.json");
/// Mermaid.js bundle
const MERMAID_JS: &[u8] = include_bytes!("../assets/mermaid@11.2.0.min.mjs");

pub struct Exporter<S>
where
    S: ExporterState,
{
    state: S,
}

pub trait ExporterState {}

impl<S> Deref for Exporter<S>
where
    S: ExporterState,
{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

// This is just a marker struct
pub struct Uninitialized {}
impl ExporterState for Uninitialized {}

pub struct Initialized {
    pub service: IntoMakeService<Router>,
}
impl ExporterState for Initialized {}

pub struct Launched {
    port: u16,
}
impl ExporterState for Launched {}

impl Exporter<Uninitialized> {
    /// Create a new exporter.
    ///
    /// # Arguments
    ///
    /// - `style` - The path to a custom CSS stylesheet to use for the exported diagram. If `None`,
    ///   the default stylesheet will be used.
    /// - `config` - The path to a custom configuration file for Mermaid.js. If `None`, the default
    ///   configuration will be used.
    /// - `diagram` - The path to the Mermaid diagram to export. If the path is `-`, the diagram
    ///   will be read from standard input.
    pub async fn new(
        style: Option<Utf8PathBuf>,
        config: Option<Utf8PathBuf>,
        diagram: Utf8PathBuf,
    ) -> Result<Exporter<Initialized>> {
        // A shared storage for resources used to serve.
        let shared_store = Arc::new(RwLock::new(Store {
            style: Self::from_file_or_default(&style, STYLE).await.to_vec(),
            config: Self::from_file_or_default(&config, CONFIG).await.to_vec(),
            diagram: {
                if &diagram == "-" {
                    let mut input = String::new();
                    let mut handle = stdin().lock();
                    handle.read_to_string(&mut input)?;
                    input.into_bytes()
                } else {
                    read(&diagram)
                        .await
                        .unwrap_or_else(|_| panic!("Failed to read input file {}", diagram))
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

        Ok(Exporter {
            state: Initialized { service: app.into_make_service() },
        })
    }

    /// Read a file from the given path or return a default value if the path is `None` or the file
    /// cannot be read.
    ///
    /// # Arguments
    ///
    /// * `path` - An optional string representing the path to a file to read.
    /// * `default` - A byte slice representing the default value to return if `path` is `None` or
    ///   the file cannot be read.
    ///
    /// # Returns
    ///
    /// A vector of bytes representing the contents of the file at the given path, or the default
    /// value if the path is `None` or the file cannot be read.
    async fn from_file_or_default<'a>(
        path: &'a Option<Utf8PathBuf>,
        default: &'a [u8],
    ) -> &'a [u8] {
        match path {
            Some(pathlike) if pathlike.exists() => {
                let content = read_to_string(&pathlike).await.unwrap_or_default();
                // Leak the content to make it have a static lifetime.
                Box::leak(content.into_boxed_str()).as_bytes()
            }
            _ => default,
        }
    }
}

impl Exporter<Initialized> {
    /// Launch the HTTP server.
    pub async fn launch(&self) -> Result<Exporter<Launched>> {
        let addr = SocketAddr::from(([127, 0, 0, 1], 0));
        let listener = TcpListener::bind(&addr).await?;
        let port = listener.local_addr()?.port();
        let service = self.service.clone();
        spawn(async {
            serve(listener, service).await.unwrap();
        });

        Ok(Exporter { state: Launched { port } })
    }
}

impl Exporter<Launched> {
    /// Export a Mermaid diagram to a file specified by the `output` argument.
    ///
    /// # Arguments
    ///
    /// - `output` - The path to the output file. The file format will be determined by the file
    ///   extension (e.g., `.png` for a PNG image, `.svg` for an SVG image).
    /// - `width` - The width of the generated image.
    /// - `height` - The height of the generated image.
    ///
    /// # Returns
    ///
    /// A string representation of the path to the output file if the export was successful, or an
    /// error if the export failed.
    pub async fn export_mermaid_to_image(
        &self,
        output: &Utf8PathBuf,
        width: u32,
        height: u32,
    ) -> Result<String> {
        let image = self.convert_mermaid_to_image(width, height, ImageFormat::from(output))?;
        write(output, image).await?;
        Ok(output.canonicalize()?.to_string_lossy().to_string())
    }

    /// Convert a Mermaid diagram to an image in the specified file format.
    ///
    /// # Arguments
    ///
    /// - `width` - The width of the generated image.
    /// - `height` - The height of the generated image.
    /// - `file_type` - The file format to use for the output image.
    /// - `port` - The port number to use for the local server serving the Mermaid diagram.
    ///
    /// # Returns
    ///
    /// A `Result` containing `Vec<u8>` representing the generated image if the export was
    /// successful, or an error if the export failed.
    fn convert_mermaid_to_image(
        &self,
        width: u32,
        height: u32,
        format: ImageFormat,
    ) -> Result<Vec<u8>> {
        let browser = Browser::new(
            LaunchOptionsBuilder::default()
                .window_size(Some((width, height)))
                .headless(true)
                .build()?,
        )?;
        let tab = browser.new_tab()?;

        tab.navigate_to(&format!("http://127.0.0.1:{}/", self.port))?;
        tab.wait_until_navigated()?;

        Ok(match format {
            ImageFormat::Svg => {
                let str = tab
                    .wait_for_element("div#mermaid")?
                    .call_js_fn(
                        &format!(
                            r#"function() {{
                            const svg = document.getElementsByTagName?.('svg')?.[0];
                            const style = document.createElementNS('http://www.w3.org/2000/svg', 'style')
                            style.appendChild(document.createTextNode({}))
                            svg.appendChild(style)
                            return new XMLSerializer().serializeToString(svg);
                        }}"#,
                            STYLE
                                .iter()
                                .copied()
                                .map(|b| b.to_string())
                                .collect::<Vec<String>>()
                                .join("")
                        ),
                        vec![],
                        true,
                    )?
                    .value
                    .ok_or(anyhow!("failed to extract SVG"))?
                    .to_string()
                    .replace(r#"\""#, r#"""#); // `this.innerHTML` returns double quoted string
                str[1..(str.len() - 1)].as_bytes().to_vec() // omit first and last "
            }
            ImageFormat::Png => tab
                .wait_for_element("div#mermaid > svg#svg")?
                .capture_screenshot(Png)?,
        })
    }
}
