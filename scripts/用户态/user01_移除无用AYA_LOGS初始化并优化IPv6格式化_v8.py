#!/usr/bin/env python3
"""
user01_移除无用AYA_LOGS初始化并优化IPv6格式化_v8.py

用途：
    1) 移除用户态 main() 里的 aya_log::EbpfLogger::init 调用。
       当前 XDP 程序没用 aya-log-ebpf 输出日志，初始化会打
       "AYA_LOGS not found" 的噪声警告，直接删掉。
    2) 优化 fmt_ipv6() 函数，按 RFC 5952 风格压缩 IPv6 地址：
       - 去掉每个 16-bit 组的前导 0
       - 用 :: 压缩最长的连续零段（至少 2 组）
       效果：0:0:0:0:0:0:0:1 → ::1

测试目标：
    事件日志可读性提升，且不再有 AYA_LOGS 噪声警告。

版本变更：
    v1  初版 fmt_ipv6（不压缩，原样输出 8 组）
    v8  移除 EbpfLogger init + RFC 5952 压缩
"""
import sys, shutil

PATH = "/root/adblock/adblock/src/main.rs"
BAK  = "/root/adblock/adblock/src/main.rs.pre_v8"

CONTENT = r'''use anyhow::Context as _;
use aya::maps::RingBuf;
use aya::programs::{Xdp, XdpMode};
use clap::Parser;
use log::{debug, info, warn};
use tokio::signal;

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long)]
    /// 要挂载 XDP 程序的网卡名（如 eth0、br0、enp0s29u1u7u1）
    iface: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::parse();
    env_logger::init();

    let rlim = libc::rlimit {
        rlim_cur: libc::RLIM_INFINITY,
        rlim_max: libc::RLIM_INFINITY,
    };
    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &rlim) };
    if ret != 0 {
        debug!("remove limit on locked memory failed, ret is: {ret}");
    }

    let mut ebpf = aya::Ebpf::load(aya::include_bytes_aligned!(concat!(
        env!("OUT_DIR"),
        "/adblock"
    )))?;

    // 不再初始化 aya_log::EbpfLogger —— 当前 XDP 程序没有用 aya-log-ebpf
    // 输出日志，初始化会报 "AYA_LOGS not found" 的噪声警告。

    let Opt { iface } = opt;
    let program: &mut Xdp = ebpf.program_mut("adblock_ebpf").unwrap().try_into()?;
    program.load()?;
    program.attach(&iface, XdpMode::default())
        .context("failed to attach the XDP program - try changing XdpMode::default() to XdpMode::Skb")?;

    // RingBuf 消费
    let ring = RingBuf::try_from(ebpf.take_map("EVENTS").context("EVENTS map not found")?)?;
    let mut async_fd = tokio::io::unix::AsyncFd::new(ring)?;

    tokio::task::spawn(async move {
        loop {
            let mut guard = match async_fd.readable_mut().await {
                Ok(g) => g,
                Err(e) => {
                    warn!("ringbuf wait failed: {e}");
                    break;
                }
            };
            while let Some(item) = guard.get_inner_mut().next() {
                let data: &[u8] = item.as_ref();
                if data.len() >= 32 {
                    let ts_ns = u64::from_ne_bytes(data[16..24].try_into().unwrap());
                    let pkt_len = u32::from_ne_bytes(data[24..28].try_into().unwrap());
                    info!(
                        "EVENT src={} ts_ns={} pkt_len={}",
                        fmt_ipv6(&data[0..16]),
                        ts_ns,
                        pkt_len
                    );
                }
            }
            guard.clear_ready();
        }
    });

    println!("Waiting for Ctrl-C...");
    signal::ctrl_c().await?;
    println!("Exiting...");
    Ok(())
}

/// 把 16 字节 IPv6 地址格式化成 RFC 5952 风格的字符串：
/// - 去掉每个 16-bit 组的前导 0
/// - 用 :: 压缩最长的连续零段（至少 2 组）
fn fmt_ipv6(b: &[u8]) -> String {
    if b.len() < 16 {
        return "?".into();
    }
    let mut groups = [0u16; 8];
    for i in 0..8 {
        groups[i] = u16::from_be_bytes([b[i * 2], b[i * 2 + 1]]);
    }
    // 找最长连续零段
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut cur_start = 0usize;
    let mut cur_len = 0usize;
    for i in 0..8 {
        if groups[i] == 0 {
            if cur_len == 0 {
                cur_start = i;
            }
            cur_len += 1;
            if cur_len > best_len {
                best_len = cur_len;
                best_start = cur_start;
            }
        } else {
            cur_len = 0;
        }
    }
    if best_len < 2 {
        best_len = 0;
    }

    let mut s = String::new();
    let mut i = 0;
    while i < 8 {
        if best_len > 0 && i == best_start {
            s.push_str("::");
            i += best_len;
            continue;
        }
        if !s.is_empty() && !s.ends_with(':') {
            s.push(':');
        }
        s.push_str(&format!("{:x}", groups[i]));
        i += 1;
    }
    if s.is_empty() {
        s.push_str("::");
    }
    s
}
'''

# 备份
shutil.copy(PATH, BAK)

# 写入新内容
with open(PATH, "w") as f:
    f.write(CONTENT)
print("OK: v8 写入成功")
print(f"备份: {BAK}")
