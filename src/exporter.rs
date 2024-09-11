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
    /// - `diagram` - The path to the Mermaid diagram to export. If the path is `-`, the diagram
    ///   will be read from standard input.
    /// - `style` - The path to a custom CSS stylesheet to use for the exported diagram. If `None`,
    ///   the default stylesheet will be used.
    /// - `config` - The path to a custom configuration file for Mermaid.js. If `None`, the default
    ///   configuration will be used. See [Interface: MermaidConfig](https://mermaid.js.org/config/setup/interfaces/mermaid.MermaidConfig.html) for more information.
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
    /// - `format` - [`ImageFormat`] enum representing the file format to use for the generated.
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
    use camino::Utf8PathBuf;
    use sha2_hasher::Sha2Hasher;
    use tokio::fs::remove_file;

    use crate::exporter::Exporter;

    macro_rules! run_test {
        ($name:ident, $hash:literal) => {
            #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
            async fn $name() {
                test(stringify!($name), $hash).await;
            }
        };
    }

    run_test!(architecture, "24725a8534450d9f0dcba0bb0ce3b0eb68dbb81e71684ed5ba57fe1ed15c8947");
    run_test!(block_diagram, "6c181056b1f10142750111a8cae126b591af60a593e9c5f5e7e877214ba21205");
    run_test!(c4_diagram, "f5da275365ff2603c31b50099ee8009bfd8d093d66713b862f6583211787a264");
    run_test!(class_diagram, "b3d76df0a3a760a99ab3c81fbbd4fb4f9991e3cace5607a15e2fa069286def3b");
    run_test!(er_diagram, "f497b79feb3792a7574959e06a44b6fce3fd1c77af3eba5549832bcc8df248c4");
    run_test!(flowchart, "52a9ebb72af7f05d2b9c418de7eb5a9dd48451b70eb420ab42e1fa4b47b0d81d"); // not looking good, though
    run_test!(gantt, "a58369afc4d07420233b72dd307ab98a6b4c4d091ef57cdaead2faa4dc3ce42d");
    run_test!(git_graph, "7c7430d4db513b057424d2662c92f8ffe82b23ac4d0c4dd9b422a2e07988d5d5");
    run_test!(mindmaps, "e7c072e4f696225e0cc95058a36afc2bda0f99419c7ca04f07b8e91ab9a65770");
    run_test!(packet, "1606b7e009594b9c7edbb29f0f404b1434e75f0e04bddb19f7e645664796db9a");
    run_test!(pie_chart, "de25e7cf1f28c44b1f4c7f82f9ef231c9d2bf036a933197542597050b4408466");
    run_test!(quadrant_chart, "559d16a223a3f4ef1d1f23a60c1b45188656606a4b595e9f07380c39f1ff5157");
    run_test!(req_diagram, "f7dd0d6e708e6591b9c20b347e25fc31102da219eac6569b201b51b5ab4c32f8");
    run_test!(sankey, "638cc725049f3c03f2844dd1b90f63167aa2ba8b5b8beb61f69f9a67194ef9aa");
    run_test!(sequence_diagram, "880497e8a7e6e6b99cbad8558e76245606b0730dd197f3299169e9478112df44");
    run_test!(state_diagram, "ae24a640f8309aab40817981c7f207eb3b065f84b3c60cde173195edf4cd8043");
    run_test!(timeline, "480ed8955001391e78680ee06c2de059244f6077473a76ce7e664203551fe329");
    run_test!(user_journey, "35ad9065fee67e10720471a02f239850f4f87906f4e56de8a82fbcb19486649a");
    run_test!(xy_chart, "025db9f1f2417bcacb66efef19bcf0c1ce2f960e573cb609c8c9efb37bd4fe62");

    async fn test(name: &str, hash: &str) {
        let input = Utf8PathBuf::from(format!("tests/fixtures/{name}.mmd").as_str());
        let output = Utf8PathBuf::from(format!("tests/fixtures/{name}.png").as_str());

        let exporter = Exporter::new(&input, None, None).await.unwrap();
        let exporter = exporter.launch().await.unwrap();
        exporter.export_mermaid_to_image(&output, 1960, 2160).await.unwrap();

        let calculated_hash = &output.sha256().unwrap();
        assert_eq!(calculated_hash, hash);

        remove_file(&output).await.unwrap(); // remove only if the assertion passes
    }
}
