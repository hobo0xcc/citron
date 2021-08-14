# Citron OS

Citron is an operating system written in Rust. Currently supports RISC-V virt machine.

## Require

- Rust (nightly)
- qemu

### Rust

1. [https://www.rust-lang.org/ja/tools/install](https://www.rust-lang.org/ja/tools/install)
2. `rustup install nightly`

### macOS

```bash
brew install qemu
```

### Ubuntu

```bash
$ sudo apt install -y qemu qemu-system-misc
```

## Build

```bash
$ make
```

## Run (qemu)

```bash
$ make qemu
```