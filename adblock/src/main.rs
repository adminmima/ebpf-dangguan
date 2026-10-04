use anyhow::Context as _;
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

    match aya_log::EbpfLogger::init(&mut ebpf) {
        Err(e) => {
            warn!("failed to initialize eBPF logger: {e}");
        }
        Ok(logger) => {
            let mut logger =
                tokio::io::unix::AsyncFd::with_interest(logger, tokio::io::Interest::READABLE)?;
            tokio::task::spawn(async move {
                loop {
                    let mut guard = logger.readable_mut().await.unwrap();
                    guard.get_inner_mut().flush();
                    guard.clear_ready();
                }
            });
        }
    }

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

fn fmt_ipv6(b: &[u8]) -> String {
    if b.len() < 16 {
        return "?".into();
    }
    let mut s = String::new();
    for i in 0..8 {
        let w = u16::from_be_bytes([b[i * 2], b[i * 2 + 1]]);
        s.push_str(&format!("{:x}", w));
        if i < 7 {
            s.push(':');
        }
    }
    s
}
