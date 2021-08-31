# Citron OS

Citron is an operating system written in Rust. Currently supports RISC-V virt machine.

## Require

- Rust (nightly)
- qemu
- dosfstools
- [riscv-gnu-toolchain](https://github.com/riscv/riscv-gnu-toolchain)

### Rust

1. [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install)
2. `rustup toolchain install nightly && rustup default nightly`

### macOS

```bash
brew install qemu dosfstools
```

### Ubuntu

```bash
$ sudo apt install -y qemu qemu-system-misc dosfstools
```

### riscv-gnu-toolchain

```bash
$ git clone https://github.com/riscv/riscv-gnu-toolchain
$ cd riscv-gnu-toolchain

# on ubuntu
$ sudo apt-get install autoconf automake autotools-dev curl python3 libmpc-dev libmpfr-dev libgmp-dev gawk build-essential bison flex texinfo gperf libtool patchutils bc zlib1g-dev libexpat-dev

# on macos
$ brew install python3 gawk gnu-sed gmp mpfr libmpc isl zlib expat
$ brew tap discoteq/discoteq
$ brew install flock

# if you choose /opt/riscv as install directory
$ ./configure --prefix=/opt/riscv
$ make
# add /opt/riscv/bin to your PATH
```

## Build

```bash
$ make
```

## Run (qemu)

```bash
$ make qemu
```