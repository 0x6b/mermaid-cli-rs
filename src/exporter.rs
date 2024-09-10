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
        diagram: &Utf8PathBuf,
        style: Option<Utf8PathBuf>,
        config: Option<Utf8PathBuf>,
    ) -> Result<Exporter<Initialized>> {
        // A shared storage for resources used to serve.
        let shared_store = Arc::new(RwLock::new(Store {
            style: Self::from_file_or_default(&style, STYLE).await.to_vec(),
            config: Self::from_file_or_default(&config, CONFIG).await.to_vec(),
            diagram: {
                if diagram == "-" {
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
    pub async fn export_mermaid_to_image(
        &self,
        output: &Utf8PathBuf,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let image = self.convert_mermaid_to_image(width, height, ImageFormat::from(output))?;
        write(output, image).await?;
        Ok(())
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

#[cfg(test)]
mod test {
    use std::path::Path;

    use anyhow::Result;
    use camino::Utf8PathBuf;
    use sha2::{Digest, Sha256};
    use tokio::fs::{read, remove_file};

    use crate::exporter::Exporter;

    macro_rules! run_test {
        ($name:ident, $hash:literal) => {
            #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
            async fn $name() {
                test(stringify!($name), $hash).await;
            }
        };
    }

    run_test!(architecture, "e85dd5164f3aec31e9dddc8059ee2b17f6cfd4d6837581db3937987a1b6c5fc0");
    run_test!(block_diagram, "90e26eb30344f2ae58431fa344967bc153bfe24c456961c03e7c276a98d1a728");
    run_test!(c4_diagram, "35767c08e3f051645d50889cdb52d2ea30fe7b64b95849ec504ecd8cc77b425a");
    run_test!(class_diagram, "8db6c688ed8175be79494db3c3f4078923472d607029a7c5d3913acc9bf058c1");
    run_test!(er_diagram, "770951e75076edcc1f5c11a6210f673a784c270860169ee632679345a0926e74");
    run_test!(flowchart, "fcf51a9179c9997057971e28994aecbf5238f0eb248a72be4fbce06b2c183891"); // not looking good, though
    run_test!(gantt, "ac0b08e06228e981bcac210dd1288c43882ebcefc01c0a1d35e8a518e8afe50e");
    run_test!(git_graph, "d1997271f23a758f9c90ba563ec0b8b32ad0509f85102ee1e40a5a1e5dbe3792");
    run_test!(mindmaps, "96f3650f393cc6e5df8fd682427171d941e379b425a9ff4b1fc71b296e37735f");
    run_test!(packet, "1606b7e009594b9c7edbb29f0f404b1434e75f0e04bddb19f7e645664796db9a");
    run_test!(pie_chart, "b91906f72effb4e5efa8f049c5a0b9c787fda5f59b80bfbb900e82a2efa2ce16");
    run_test!(quadrant_chart, "c9369f8d6e69ed440042197fdb2583b97d6c2f9f0dddbdd8de8f697d0c991abf");
    run_test!(req_diagram, "59f5c1de673548bb0f1fd962c9156ec98721196d53dc7f032ee483439e1928bd");
    run_test!(sankey, "4ba7c62739885a73bd86b64e1308b9bb199e95c3a8ff83e7ec5ef503a312b8fd");
    run_test!(sequence_diagram, "f81f3aaa1564291ef00ef7cd23bf84d0a5e3346abfcf4a882b5715ec09528d59");
    run_test!(state_diagram, "532a4724d32757928cdd6401902b85447b16c43a11d53bcdbc149a0aaaac304d");
    run_test!(timeline, "ff73a710037ecaec6a16058c9abf35522f12c95fae4a3e78822c1ad6a982c9ab");
    run_test!(user_journey, "632edd8f21c4ecd545851791337def8fe450ea4444dd1e80794289529de95dfd");
    run_test!(xy_chart, "807cf8ebd3b151b09cd9b5cd2fdaa73da71acf869379969ef037ef7d99820a60");

    async fn test(name: &str, hash: &str) {
        let input = Utf8PathBuf::from(format!("tests/fixtures/{name}.mmd").as_str());
        let output = Utf8PathBuf::from(format!("tests/fixtures/{name}.png").as_str());
        let exporter = Exporter::new(&input, None, None).await.unwrap();
        let exporter = exporter.launch().await.unwrap();
        exporter.export_mermaid_to_image(&output, 1960, 2160).await.unwrap();
        let calculated_hash = calculate_hash(&output).await.unwrap();
        assert_eq!(calculated_hash, hash);
        remove_file(&output).await.unwrap();
    }

    async fn calculate_hash<P>(path: P) -> Result<String>
    where
        P: AsRef<Path>,
    {
        let mut hasher = Sha256::new();
        hasher.update(read(&path).await?);
        let hash = hasher.finalize();
        let hex = hash.iter().fold(String::new(), |mut output, b| {
            output.push_str(&format!("{b:02x}"));
            output
        });
        Ok(hex)
    }
}
