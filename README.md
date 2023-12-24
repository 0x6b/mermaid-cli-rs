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
Convert Mermaid diagram to PNG or SVG format.

Usage: mmdc [OPTIONS] --input <DIAGRAM> --output <OUTPUT>

Options:
  -i, --input <DIAGRAM>      Path to the Mermaid diagram file. Specify `-` for stdin
  -o, --output <OUTPUT>      Path to the output file. By default, the file format is PNG. Specify a `.svg` extension if you need an SVG file
  -w, --width <WIDTH>        Width of the output image in pixels [default: 1960]
  -H, --height <HEIGHT>      Height of the output image in pixels. This value is automatically reduced to fit the image [default: 2160]
  -c, --cssFile <STYLE>      Path to a CSS file for the HTML page
  -C, --configFile <CONFIG>  Path to a JSON configuration file for Mermaid
  -f, --font <FONT>          Path to a font file for Mermaid
  -h, --help                 Print help
  -V, --version              Print version
```

### Benchmark

If you require better performance or more advanced capabilities, we recommend
using [mermaid-js/mermaid-cli](https://github.com/mermaid-js/mermaid-cli). You can review a rough, not scientific, benchmark I conducted with the diagram example available at [mermaid-js/mermaid/demos/flowchart.html](https://github.com/mermaid-js/mermaid/blob/4e4f2fcfc5367f22edea685b8f48ad2d7525d1c0/demos/flowchart.html).

- Node.js v18.18.0
- Rust 1.73.0 (cc66ad468 2023-10-03)
- macOS 14.0 (Sonoma)
- Apple M1 Max

```
$ hyperfine --warmup 5 './node_modules/.bin/mmdc -i test.mmd -o test.png' './target/release/mmdc -i test.mmd -o test.png'
Benchmark 1: ./node_modules/.bin/mmdc -i test.mmd -o test.png
  Time (mean ± σ):      1.418 s ±  0.013 s    [User: 0.394 s, System: 0.161 s]
  Range (min … max):    1.398 s …  1.446 s    10 runs
 
Benchmark 2: ./target/release/mmdc -i test.mmd -o test.png
  Time (mean ± σ):      3.833 s ±  0.146 s    [User: 0.184 s, System: 0.104 s]
  Range (min … max):    3.625 s …  4.059 s    10 runs
 
Summary
  './node_modules/.bin/mmdc -i test.mmd -o test.png' ran
    2.70 ± 0.11 times faster than './target/release/mmdc -i test.mmd -o test.png'
```

## Licenses

The binary embeds following assets during build. See respective LICENSE for details.

- [Source Han Sans font](https://github.com/adobe-fonts/source-han-sans/raw/release/Variable/WOFF2/OTF/Subset/SourceHanSansJP-VF.otf.woff2) (variable Japanese): [LICENSE](https://raw.githubusercontent.com/adobe-fonts/source-han-sans/master/LICENSE.txt)
- [Mermaid v10.6.1](https://github.com/mermaid-js/mermaid/tree/v10.6.1): [LICENSE](https://github.com/mermaid-js/mermaid/blob/v10.6.1/LICENSE)

Others are released under the MIT License. See [LICENSE](LICENSE) for details.
