# mermaid-cli-rs

A naive Rust port of [mermaid-js/mermaid-cli](https://github.com/mermaid-js/mermaid-cli), a command-line tool for the [Mermaid](https://mermaid.js.org/), for easy deployment.

Tested on Apple Silicon but might be cross-platform.

## Install

Run the following command:

```shell
$ cargo install https://github.com/0x6b/mermaid-cli-rs
```

## Usage

The CLI supports the following command-line options:

```
USAGE:
    mmdc [OPTIONS] --input <diagram> --output <output>

FLAGS:
        --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -C, --configFile <config>    Path to a JSON configuration file for Mermaid
    -i, --input <diagram>        Path to the Mermaid diagram file
    -f, --font <font>            Path to a font file for Mermaid
    -h, --height <height>        Height of the output image in pixels. This value is automatically reduced to fit the
                                 image [default: 2160]
    -o, --output <output>        Path to the output file. By default, the file format is PNG. Use a `.svg` extension for
                                 an SVG file
    -c, --cssFile <style>        Path to a CSS file for the HTML page
    -w, --width <width>          Width of the output image in pixels [default: 1960]
```

### Benchmark

If you require better performance or more advanced capabilities, we recommend
using [mermaid-js/mermaid-cli](https://github.com/mermaid-js/mermaid-cli). You can review a rough, not scientific, benchmark I conducted with the diagram example available at [mermaid-js/mermaid/demos/flowchart.html](https://github.com/mermaid-js/mermaid/blob/4e4f2fcfc5367f22edea685b8f48ad2d7525d1c0/demos/flowchart.html).

- Node.js v18.14.2
- Rust 1.67.1 (d5a82bbd2 2023-02-07)
- macOS 13.2.1 (Ventura)
- Apple M1 Max

```
$ hyperfine --warmup 5 './node_modules/.bin/mmdc -i test.mmd -o test.png' './target/release/mmdc -i test.mmd -o test.png'
Benchmark 1: ./node_modules/.bin/mmdc -i test.mmd -o test.png
  Time (mean ± σ):      1.517 s ±  0.044 s    [User: 0.409 s, System: 0.163 s]
  Range (min … max):    1.477 s …  1.620 s    10 runs
 
Benchmark 2: ./target/release/mmdc -i test.mmd -o test.png
  Time (mean ± σ):      3.960 s ±  0.126 s    [User: 0.206 s, System: 0.114 s]
  Range (min … max):    3.737 s …  4.118 s    10 runs
 
Summary
  './node_modules/.bin/mmdc -i test.mmd -o test.png' ran
    2.61 ± 0.11 times faster than './target/release/mmdc -i test.mmd -o test.png'
```

## Licenses

The binary embeds following assets during build. See respective LICENSE for details.

- [Source Han Sans font](https://github.com/adobe-fonts/source-han-sans/raw/release/Variable/WOFF2/OTF/Subset/SourceHanSansJP-VF.otf.woff2) (variable Japanese): [LICENSE](https://raw.githubusercontent.com/adobe-fonts/source-han-sans/master/LICENSE.txt)
- [Mermaid v9.4.0](https://github.com/mermaid-js/mermaid/tree/v9.4.0): [LICENSE](https://raw.githubusercontent.com/mermaid-js/mermaid/v9.4.0/LICENSE)

Others are released under the MIT License. See [LICENSE](LICENSE) for details.
