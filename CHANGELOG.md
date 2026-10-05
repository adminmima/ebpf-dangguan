# Changelog

遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/) 格式，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added
- eBPF XDP 程序骨架：以太头 / IPv4 / IPv6 解析
- IPv6 扩展头解析（Hop-by-Hop / Routing / DestOpts / AH / Fragment），fail-closed
- LPM Trie 双向 IP 黑名单（IPv4 / IPv6）
- RingBuf 内核态→用户态事件通路（tokio 异步消费）
- `--verify` 短路模式：load → attach → detach → exit
- Makefile：build / ebpf / verifier / clippy / fmt / test / check
- CI：clippy + fmt + build + test

### Changed
- verifier target 放弃 bpftool/libbpf，改用 aya 自身加载
  （依据：aya-rs/aya#913，标准 #[map] 宏与 libbpf 不兼容）

### Fixed
- clippy workspace unwinding panic（拆成 host / ebpf 两个 target）

## [0.1.0] - 2026-10-04

### Added
- 初始项目骨架（aya-template 生成）
- Rust toolchain 钉版 nightly-2026-09-29
- bpf-linker 0.11.1
