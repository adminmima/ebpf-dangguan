# ebpf-dangguan

> 一夫当关，万夫莫开 — *One guard at the pass, ten thousand cannot get through.*

eBPF/XDP-based DNS ad blocker and pure-IP direct-connection detector for Linux home routers.

[中文文档](README.zh-CN.md) · [Handover / Roadmap](HANDOVER.md)

## Status

**Early development.** Core infrastructure is complete; DNS filtering and pure-IP detection logic are in progress.

## Design goals

- **Large-scale DNS ad blocking** — up to millions of domains, O(1) hash lookup in XDP
- **Pure-IP direct-connection detection** — flag connections that bypass DNS resolution
- **Full IPv6 support** — extension-header parsing with a fail-closed policy, no IPv6 bypass
- **Hot-reloadable blocklists** — no service restart required

## Current capabilities

| Capability | Status |
|---|---|
| XDP Ethernet / IPv4 / IPv6 parsing | done |
| IPv6 extension headers + fail-closed | done |
| LPM Trie IP blocklist (v4 / v6) | done |
| RingBuf kernel → user event channel | done |
| DNS QNAME extraction | in progress |
| Domain blocklist + drop | planned |
| Pure-IP detection (TC egress) | planned |

See [HANDOVER.md](HANDOVER.md) for the full capability list and design decisions.

## Requirements

| Component | Version |
|---|---|
| Linux kernel | >= 5.8 (RingBuf) |
| Rust toolchain | nightly-2026-09-29 (pinned in `rust-toolchain.toml`) |
| bpf-linker | 0.11.1 |
| bpftool | recent (for local verifier) |

## Build

    make build        # compile workspace (eBPF via aya-build)
    make check        # clippy + fmt + build + test + verifier
    make verifier     # local verifier (needs root + /sys/fs/bpf)

## Run

    sudo RUST_LOG=info cargo run --release -p adblock -- --iface <iface>

Events are reported via RingBuf and printed to stdout.

## Verify without attaching

Load, attach, check maps, detach, exit — no packet loop:

    sudo cargo run --release -p adblock -- --iface lo --verify

## License

- **User space** (`adblock`, `adblock-common`): AGPL-3.0-or-later
- **eBPF** (`adblock-ebpf`): Dual MIT/GPL — the kernel validates the ELF license section on `bpf_prog_load`, requiring a GPL-compatible string (this program uses the GPL-only helper `bpf_ktime_get_ns`)

Project skeleton based on [aya-rs/aya-template](https://github.com/aya-rs/aya-template) (MIT OR Apache-2.0). See [NOTICE](NOTICE).

## Name

**dangguan** (当关) — from *一夫当关，万夫莫开*: one guard at the pass, ten thousand cannot get through.
