# ebpf-dangguan

> 一夫当关，万夫莫开

基于 eBPF/XDP 的 DNS 广告拦截与纯 IP 直连识别系统，面向 Linux 家庭路由器。

[English](README.md) · [交接文档 / 路线图](HANDOVER.md)

## 状态

**早期开发中。** 核心基建已完成；DNS 过滤与纯 IP 识别逻辑正在实现。

## 设计目标

- **大规模 DNS 广告拦截** — 支持百万级域名，XDP 内 O(1) 哈希查找
- **纯 IP 直连识别** — 标记绕过 DNS 解析、直接通过 IP 发起的连接
- **完整 IPv6 支持** — 扩展头解析采用 fail-closed 策略，不给 IPv6 留绕过通道
- **黑名单热更新** — 无需重启服务

## 当前能力

| 能力 | 状态 |
|---|---|
| XDP 以太头 / IPv4 / IPv6 解析 | 已完成 |
| IPv6 扩展头 + fail-closed | 已完成 |
| LPM Trie IP 黑名单（v4 / v6） | 已完成 |
| RingBuf 内核→用户态事件通路 | 已完成 |
| DNS QNAME 提取 | 进行中 |
| 域名黑名单 + 丢包 | 计划中 |
| 纯 IP 检测（TC egress） | 计划中 |

完整能力清单与设计定案见 [HANDOVER.md](HANDOVER.md)。

## 环境要求

| 组件 | 版本 |
|---|---|
| Linux 内核 | >= 5.8（RingBuf 硬要求） |
| Rust 工具链 | nightly-2026-09-29（见 `rust-toolchain.toml`） |
| bpf-linker | 0.11.1 |
| bpftool | 近期版本（本地 verifier 用） |

## 构建

    make build        # 编译 workspace（eBPF 由 aya-build 触发）
    make check        # clippy + fmt + build + test + verifier
    make verifier     # 本地 verifier（需 root + /sys/fs/bpf 挂载）

## 运行

    sudo RUST_LOG=info cargo run --release -p adblock -- --iface <网卡名>

事件经 RingBuf 上报，打印到 stdout。

## 验证（不挂载收包）

加载 → attach → map 检查 → detach → 退出：

    sudo cargo run --release -p adblock -- --iface lo --verify

## License

- **用户态**（`adblock`、`adblock-common`）：AGPL-3.0-or-later
- **eBPF**（`adblock-ebpf`）：Dual MIT/GPL — 内核在 `bpf_prog_load` 时校验 ELF license section，需 GPL 兼容字符串（本程序用到 GPL-only helper `bpf_ktime_get_ns`）

项目骨架源自 [aya-rs/aya-template](https://github.com/aya-rs/aya-template)（MIT OR Apache-2.0），详见 [NOTICE](NOTICE)。

## 命名

**dangguan**（当关）— 取"一夫当关，万夫莫开"之意。
