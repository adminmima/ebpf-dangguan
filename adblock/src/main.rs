use adblock_common::DnsEvent;
use anyhow::Context as _;
use aya::{
    maps::RingBuf,
    programs::{Xdp, XdpMode},
};
use clap::Parser;
use log::{debug, info, warn};
use tokio::signal;

// 布局锁：与内核侧 adblock-common 一致
const _: () = assert!(core::mem::size_of::<DnsEvent>() == 296);

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long)]
    /// 要挂载 XDP 程序的网卡名（如 eth0、br0、enp0s29u1u7u1）
    iface: String,

    /// 验证模式：加载 → attach → 检查 map → detach → exit(0)
    #[clap(long)]
    verify: bool,
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

    if opt.verify {
        if let Err(e) = run_verify(&mut ebpf, &opt.iface) {
            eprintln!("verify FAILED: {e:#}");
            std::process::exit(1);
        }
        println!("verify OK");
        std::process::exit(0);
    }

    let Opt { iface, .. } = opt;
    let program: &mut Xdp = ebpf.program_mut("adblock_ebpf").unwrap().try_into()?;
    program.load()?;
    program.attach(&iface, XdpMode::default()).context(
        "failed to attach the XDP program - try changing XdpMode::default() to XdpMode::Skb",
    )?;

    // —— ICMPv6 事件流（原有）——
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

    // —— DNS QNAME 事件流 ——
    let dns_ring = RingBuf::try_from(
        ebpf.take_map("DNS_EVENTS")
            .context("DNS_EVENTS map not found")?,
    )?;
    let mut dns_fd = tokio::io::unix::AsyncFd::new(dns_ring)?;
    tokio::task::spawn(async move {
        loop {
            let mut guard = match dns_fd.readable_mut().await {
                Ok(g) => g,
                Err(e) => {
                    warn!("dns ringbuf wait failed: {e}");
                    break;
                }
            };
            while let Some(item) = guard.get_inner_mut().next() {
                let data: &[u8] = item.as_ref();
                if data.len() < 296 {
                    continue;
                }
                let src_ip = &data[0..16];
                let dst_ip = &data[16..32];
                let src_port = u16::from_ne_bytes(data[32..34].try_into().unwrap());
                let dst_port = u16::from_ne_bytes(data[34..36].try_into().unwrap());
                let qlen = u16::from_ne_bytes(data[36..38].try_into().unwrap()) as usize;
                let ip_ver = data[38];
                if ip_ver != 4 && ip_ver != 6 {
                    continue;
                }
                let qlen = qlen.min(256);
                if data.len() < 40 + qlen {
                    continue;
                }
                let raw_qname = &data[40..40 + qlen];
                let Some(domain) = parse_dns_qname(raw_qname) else {
                    // 畸形 / 压缩指针 / 根域名 —— eBPF 侧已 bump S_DNS，这里静默丢弃
                    continue;
                };
                info!(
                    "[QNAME] {} (len={}) src={}:{} dst={}:{} ver={}",
                    domain,
                    domain.len(),
                    fmt_ip(ip_ver, src_ip),
                    src_port,
                    fmt_ip(ip_ver, dst_ip),
                    dst_port,
                    ip_ver,
                );
            }
            guard.clear_ready();
        }
    });

    println!("Waiting for Ctrl-C...");
    signal::ctrl_c().await?;
    println!("Exiting...");
    Ok(())
}

fn run_verify(ebpf: &mut aya::Ebpf, iface: &str) -> anyhow::Result<()> {
    for name in [
        "STATS",
        "EVENTS",
        "DNS_EVENTS",
        "IP_BLOCKLIST_V4",
        "IP_BLOCKLIST_V6",
    ] {
        if ebpf.map(name).is_none() {
            anyhow::bail!("map {name} not found");
        }
        eprintln!("  map {name}: OK");
    }

    let program: &mut Xdp = ebpf
        .program_mut("adblock_ebpf")
        .ok_or_else(|| anyhow::anyhow!("program adblock_ebpf not found"))?
        .try_into()?;
    program.load().context("XDP load failed")?;
    eprintln!("  program load: OK");

    let link_id = program
        .attach(iface, XdpMode::default())
        .with_context(|| format!("XDP attach to {iface} failed"))?;
    eprintln!("  XDP attach to {iface}: OK");

    program.detach(link_id).context("XDP detach failed")?;
    eprintln!("  XDP detach: OK");

    Ok(())
}

/// 从 DNS 报文（12B 头 + Question section）解析首个 QNAME。
///
/// 输入是 eBPF 侧从 Question section 起点定长拷贝的原始字节
/// （含 label 长度前缀和末尾 0）。
///
/// 返回 Some(域名，不含末尾点)；Err/None 表示：
///   - 首字节 0x00 根域名
///   - 压缩指针 0xC0
///   - label 超 63
///   - 累积超 255
///   - 拷贝被截断（label 内容越过 raw 边界）
fn parse_dns_qname(raw: &[u8]) -> Option<String> {
    // raw 是 DNS 报文起点（12B 固定头 + Question section）
    if raw.len() < 12 {
        return None;
    }
    // QDCOUNT 必须 == 1
    let qdcount = u16::from_be_bytes([raw[4], raw[5]]);
    if qdcount != 1 {
        return None;
    }

    let mut out = String::new();
    let mut pos = 12usize;
    let mut total = 0usize;
    loop {
        let len = *raw.get(pos)? as usize;
        pos += 1;
        if len == 0 {
            if out.is_empty() {
                return None; // 根域名
            }
            return Some(out);
        }
        if len & 0xC0 != 0 {
            return None; // 压缩指针
        }
        if len > 63 {
            return None;
        }
        if total + len + 1 > 255 {
            return None;
        }
        let label = raw.get(pos..pos + len)?;
        if !out.is_empty() {
            out.push('.');
        }
        out.push_str(&String::from_utf8_lossy(label));
        total += len + 1;
        pos += len;
    }
}

/// ver=4 用前 4 字节点分十进制，ver=6 走 fmt_ipv6。
fn fmt_ip(ip_ver: u8, b: &[u8]) -> String {
    if ip_ver == 4 && b.len() >= 4 {
        format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3])
    } else if b.len() >= 16 {
        fmt_ipv6(&b[..16])
    } else {
        "?".into()
    }
}

fn fmt_ipv6(b: &[u8]) -> String {
    if b.len() < 16 {
        return "?".into();
    }
    let mut groups = [0u16; 8];
    for i in 0..8 {
        groups[i] = u16::from_be_bytes([b[i * 2], b[i * 2 + 1]]);
    }
    let mut best_start = 0usize;
    let mut best_len = 0usize;
    let mut cur_start = 0usize;
    let mut cur_len = 0usize;
    for (i, g) in groups.iter().enumerate() {
        if *g == 0 {
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
