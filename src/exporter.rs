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
const MERMAID_JS: &[u8] = include_bytes!("../assets/mermaid@11.2.1.min.mjs");

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
                "/{path}",
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
                str.as_bytes()[1..(str.len() - 1)].to_vec() // omit first and last "
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

    run_test!(architecture, "b32fad8246aff84540c356aa677b14a0a13fcf209a86b62fb3badbe7ac4f123e");
    run_test!(block_diagram, "05893a4b8769f67f7fb107ca2e7ca06ee65a1230d668a23ad0271f824adb474b");
    run_test!(c4_diagram, "1af3052f55cf2754d23277c894d98287e718d6b7293df1d240d166470a5b2bb0");
    run_test!(class_diagram, "302d11f4c74f0ae0268ccb695d1b7545161e64f079fa22b91517c46db74121bf");
    run_test!(er_diagram, "65f5f8d329a9d2140cd9acc0b5a17274bbbb66fb9fa4e6856b1a67b9c30041a9");
    run_test!(flowchart, "a42a2a4e5b5a7df5ff71f6b23d4f10af78f7e95c5e82b0a65b0554c5f51dc6c3"); // not looking good, though
    run_test!(gantt, "0533af17296c01261188059bdec1060f1efe1ad4d8ef28638ecf1e16b51fe12a");
    run_test!(git_graph, "7e94278d7e2ddb09edad7334a776a362905e8fde915509c6682cde9b4396f31c");
    run_test!(mindmaps, "caa0bc55871cebc4a258d42ad16c5fc0dfebbb37aaa4e3ee46e511399d5f4735");
    run_test!(packet, "bdc42435f9ed8692cfd1b141536ed00a66a03b9263ea356c68d9eba0628c5600");
    run_test!(pie_chart, "fe8900ddb8b4b135a3b2b4b091cbbdfb9c2e40fcc0ca80da2bf967d5bb4c6f48");
    run_test!(quadrant_chart, "86a1ca6d775f019c0214e4d1238c67aa7269d3cfc76457cea2f53920b0ad31b1");
    run_test!(req_diagram, "193d3e6ea430aa3a5816593ee3a85ad891b22cf8bc9034b6f6b241e927386d9a");
    run_test!(sankey, "8c818247324b500c3da21b95238b077bc3c85cf0519304aa6acdb80ce064426c");
    run_test!(sequence_diagram, "b4162d1031f0645245444984af2836728beb108ddf0221ce87ccbfacca4d729d");
    run_test!(state_diagram, "bc427ffcf770c02c79f2b58f9c314fd2f34146e66faef593629d0e9ff0c827e0");
    run_test!(timeline, "df4c9518b4ed340c0f03add0b90365f2e30f429766c6e642e4b9f0fa00915501");
    run_test!(user_journey, "2e19aa2cfbad61e3b37022930d2cccf2c9bf3aa3225bf073103db772b775da66");
    run_test!(xy_chart, "a35453d5cfb83241f91a6be157a13cf993e53d4ad4b9921098a697620c71deb6");

    async fn test(name: &str, hash: &str) {
        let input = Utf8PathBuf::from(format!("tests/fixtures/{name}.mmd").as_str());
        let output = Utf8PathBuf::from(format!("tests/fixtures/{name}.png").as_str());

        let exporter = Exporter::new(&input, None, None).await.unwrap();
        let exporter = exporter.launch().await.unwrap();
        exporter.export_mermaid_to_image(&output, 1960, 2160).await.unwrap();

        let calculated_hash = &output.sha256().await.unwrap();
        assert_eq!(calculated_hash, hash);

        remove_file(&output).await.unwrap(); // remove only if the assertion passes
    }
}
