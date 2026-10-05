# adblock

## Prerequisites

1. stable rust toolchains: `rustup toolchain install stable`
1. nightly rust toolchains: `rustup toolchain install nightly --component rust-src`
1. (if cross-compiling) rustup target: `rustup target add ${ARCH}-unknown-linux-musl`
1. (if cross-compiling) LLVM: (e.g.) `brew install llvm` (on macOS)
1. bpf-linker: `cargo install bpf-linker` (`--no-default-features` on macOS)

## Build & Run

Use `cargo build`, `cargo check`, etc. as normal. Run your program with:

```shell
cargo run --release
```

Cargo build scripts are used to automatically build the eBPF correctly and include it in the
program.

## Cross-compiling on macOS

Cross compilation should work on both Intel and Apple Silicon Macs.

```shell
cargo build --package adblock --release \
  --target=${ARCH}-unknown-linux-musl \
  --config=target.${ARCH}-unknown-linux-musl.linker=\"rust-lld\"
```
The cross-compiled program `target/${ARCH}-unknown-linux-musl/release/adblock` can be
copied to a Linux server or VM and run there.

## License

With the exception of eBPF code, adblock is distributed under the terms
of either the [MIT license] or the [Apache License] (version 2.0), at your
option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

### eBPF

All eBPF code is distributed under either the terms of the
[GNU General Public License, Version 2] or the [MIT license], at your
option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the GPL-2 license, shall be
dual licensed as above, without any additional terms or conditions.

[Apache license]: LICENSE-APACHE
[MIT license]: LICENSE-MIT
[GNU General Public License, Version 2]: LICENSE-GPL2


---

## 开发环境要求

- Linux 内核 >= 5.8（RingBuf 硬要求；更低内核需改用 PerfEventArray）
- Rust 工具链 nightly-2026-09-29（见 rust-toolchain.toml），需 rust-src 组件
- bpf-linker 0.11.1
- bpftool（本地 verifier 检查用）

### 安装步骤

    rustup toolchain install nightly-2026-09-29 --component rust-src --component rustfmt --component clippy
    rustup target add --toolchain nightly-2026-09-29 bpfel-unknown-none
    cargo install bpf-linker --locked --version 0.11.1

### 版本对齐规则

- aya / aya-ebpf / aya-log / aya-log-ebpf 必须来自同一 release 线（当前 0.2.x）
- 升级 bpf-linker 可能改变 LLVM 行为，升级前跑 make check

### 本地验证

    make check     # clippy + fmt + build + test + verifier
    make verifier  # 仅 verifier（需 root + /sys/fs/bpf 挂载）

CI 只跑 clippy + fmt + build + test，不跑 verifier——GitHub Actions 默认 runner 无 BPF 权限。
