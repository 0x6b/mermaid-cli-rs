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

- Node.js v20.17.0
- @mermaid-js/mermaid-cli: 11.2.0
- Rust 1.81.0 (eeb90cda1 2024-09-04)
- macOS 15.0 (Sequoia)
- Apple M1 Max

```
$ cargo build --release
$ hyperfine --warmup 5 './node_modules/.bin/mmdc -i tests/bench.mmd -o test.png' './target/release/mmdc -i tests/bench.mmd -o test.png'
Benchmark 1: ./node_modules/.bin/mmdc -i tests/bench.mmd -o test.png
  Time (mean ± σ):      1.156 s ±  0.009 s    [User: 1.038 s, System: 0.248 s]
  Range (min … max):    1.145 s …  1.165 s    10 runs

Benchmark 2: ./target/release/mmdc -i tests/bench.mmd -o test.png
  Time (mean ± σ):      3.364 s ±  0.042 s    [User: 0.270 s, System: 0.136 s]
  Range (min … max):    3.308 s …  3.432 s    10 runs

Summary
  ./node_modules/.bin/mmdc -i tests/bench.mmd -o test.png ran
    2.91 ± 0.04 times faster than ./target/release/mmdc -i tests/bench.mmd -o test.png```
```

## Licenses

The binary embeds following asset during build. See the respective LICENSE for details.

- [Mermaid v11.2.1](https://github.com/mermaid-js/mermaid/tree/mermaid%4011.2.1): [LICENSE](https://github.com/mermaid-js/mermaid/blob/mermaid%4011.2.1/LICENSE)

Others are released under the MIT License. See [LICENSE](LICENSE) for details.
