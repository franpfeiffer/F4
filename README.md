# F4

A simple text editor with Vim motions.

## Install

### For actual OSes
install [homebrew](https://brew.sh/)

### Linux

```bash
brew tap franpfeiffer/f4
brew install f4
```

### macOS

```bash
brew tap franpfeiffer/f4
brew install --cask f4
```

### For Windows
install [scoop](https://scoop.sh/)

```powershell
scoop bucket add f4 https://github.com/franpfeiffer/scoop-f4
scoop install f4
```

## Build from source

Prerequisites:
- rust
- cargo

```bash
git clone https://github.com/franpfeiffer/F4.git
cd F4
cargo build --release
```

The binary will be at `target/release/f4`.

To install the binary, run:
```bash
git clone https://github.com/franpfeiffer/F4.git
cd F4
cargo install --path .
```
