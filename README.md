# mermaid-cli-rs

A naive Rust port of [mermaid-js/mermaid-cli](https://github.com/mermaid-js/mermaid-cli), a command-line tool for the [Mermaid](https://mermaid.js.org/), for easy deployment—no internet access, one binary file.

Tested on Apple Silicon but might be cross-platform.

## Install

Run the following command:

```shell
$ cargo install https://github.com/0x6b/mermaid-cli-rs
```

## Usage

The CLI supports the following command-line options:

```
$ mmdc --help
Convert Mermaid diagram to PNG or SVG format.

Usage: mmdc [OPTIONS] --input <DIAGRAM> --output <OUTPUT>

Options:
  -i, --input <DIAGRAM>      Path to the Mermaid diagram file. Specify `-` for stdin
  -o, --output <OUTPUT>      Path to the output file. By default, the file format is PNG. Specify a
                             `.svg` extension if you need an SVG file
  -w, --width <WIDTH>        Width of the output image in pixels [default: 1960]
  -H, --height <HEIGHT>      Height of the output image in pixels. This value is automatically reduced
                             to fit the image [default: 2160]
  -c, --cssFile <STYLE>      Path to a CSS file for the HTML page
  -C, --configFile <CONFIG>  Path to a JSON configuration file for Mermaid. See
                             https://mermaid.js.org/config/setup/interfaces/mermaid.MermaidConfig.html
                             for more information
  -h, --help                 Print help
  -V, --version              Print version
```

### Benchmark

If you require better performance or more advanced capabilities, we recommend
using [mermaid-js/mermaid-cli](https://github.com/mermaid-js/mermaid-cli). You can review a rough, not scientific, benchmark I conducted with the diagram example available at [mermaid-js/mermaid/demos/flowchart.html](https://github.com/mermaid-js/mermaid/blob/4e4f2fcfc5367f22edea685b8f48ad2d7525d1c0/demos/flowchart.html).

- Node.js v25.2.1
- @mermaid-js/mermaid-cli: 11.12.0
- Rust 1.92.0 (ded5c06cf 2025-12-08)
- macOS 26.1 (Tahoe)
- Apple M4 Pro

```
$ cargo build --release
$ hyperfine --warmup 5 './node_modules/.bin/mmdc -i tests/bench.mmd -o test.png' './target/release/mmdc -i tests/bench.mmd -o test.png'
Benchmark 1: ./node_modules/.bin/mmdc -i tests/bench.mmd -o test.png
  Time (mean ± σ):     900.5 ms ±  17.3 ms    [User: 818.6 ms, System: 213.6 ms]
  Range (min … max):   878.8 ms … 930.4 ms    10 runs

Benchmark 2: ./target/release/mmdc -i tests/bench.mmd -o test.png
  Time (mean ± σ):      3.254 s ±  0.015 s    [User: 0.174 s, System: 0.097 s]
  Range (min … max):    3.228 s …  3.274 s    10 runs

Summary
  ./node_modules/.bin/mmdc -i tests/bench.mmd -o test.png ran
    3.61 ± 0.07 times faster than ./target/release/mmdc -i tests/bench.mmd -o test.png
```

The performance gap is due to browser startup overhead in the [headless_chrome](https://crates.io/crates/headless_chrome) crate. Various Chrome launch options were tested (disabling sandbox, GPU, extensions, background networking, etc.) with no meaningful improvement—the ~3s overhead is inherent to the library's architecture. The tradeoff is: slower execution, but a single self-contained binary with no Node.js/npm dependency.

## Licenses

The binary embeds following asset during build. See the respective LICENSE for details.

- [Mermaid v11.12.2](https://github.com/mermaid-js/mermaid/tree/mermaid%4011.12.2): [LICENSE](https://github.com/mermaid-js/mermaid/blob/mermaid%4011.12.2/LICENSE)

Others are released under the MIT License. See [LICENSE](LICENSE) for details.
