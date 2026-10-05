use anyhow::Context as _;
use aya::{
    maps::RingBuf,
    programs::{Xdp, XdpMode},
};
use clap::Parser;
use log::{debug, info, warn};
use tokio::signal;

#[derive(Debug, Parser)]
struct Opt {
    #[clap(short, long)]
    /// 要挂载 XDP 程序的网卡名（如 eth0、br0、enp0s29u1u7u1）
    iface: String,

    /// 验证模式：加载 → attach → 检查 map → detach → exit(0)
    /// 不进入收包循环；任何一步失败以非零码退出
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

    // === 短路模式：--verify ===
    if opt.verify {
        if let Err(e) = run_verify(&mut ebpf, &opt.iface) {
            eprintln!("verify FAILED: {e:#}");
            std::process::exit(1);
        }
        println!("verify OK");
        std::process::exit(0);
    }

    // === 正常模式 ===
    let Opt { iface, .. } = opt;
    let program: &mut Xdp = ebpf.program_mut("adblock_ebpf").unwrap().try_into()?;
    program.load()?;
    program.attach(&iface, XdpMode::default()).context(
        "failed to attach the XDP program - try changing XdpMode::default() to XdpMode::Skb",
    )?;

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

/// 验证模式：加载 → attach → 检查 map → detach → 返回
/// 全程同步，任何一步失败立即返回 Err。
fn run_verify(ebpf: &mut aya::Ebpf, iface: &str) -> anyhow::Result<()> {
    // 1. 四个 map 存在性检查
    for name in ["STATS", "EVENTS", "IP_BLOCKLIST_V4", "IP_BLOCKLIST_V6"] {
        if ebpf.map(name).is_none() {
            anyhow::bail!("map {name} not found");
        }
        eprintln!("  map {name}: OK");
    }

    // 2. XDP 加载 + attach
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

    // 3. 立即 detach
    program.detach(link_id).context("XDP detach failed")?;
    eprintln!("  XDP detach: OK");

    Ok(())
}

/// 把 16 字节 IPv6 地址格式化成 RFC 5952 风格字符串
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
