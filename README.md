# minifs-extractor

A tool to extract `minifs` filesystems from binary files, which are common with VxWorks images.

Special thanks to **Dmitrii Belimov** and **Evgenii Vinogradov** for writing [the paper](https://arxiv.org/html/2407.05064v1) this tool was based on.

Usage:
```
minifs-extractor ./firmware.bin
```

It will create a `_firmware.bin.extracted` directory with all the files found in the filesystem.

Before installing with cargo or building from source, make sure you have rust installed:
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup
```

## Installation

* From cargo (requires rust installation):
```
cargo install minifs-extractor
```

## Building from source

Build it with cargo:
```
git clone https://github.com/RedaOps/minifs-extractor
cd minifs-extractor
cargo build --release
./target/release/minifs-extractor -h
```
