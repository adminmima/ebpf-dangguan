#![no_std]
#![no_main]

mod dns;
mod ptr;

use adblock_common::DnsEvent;
use aya_ebpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::{
        PerCpuArray, RingBuf,
        lpm_trie::{Key, LpmTrie},
    },
    programs::XdpContext,
};

#[repr(C)]
pub struct Event {
    pub src_ip: [u8; 16],
    pub ts_ns: u64,
    pub pkt_len: u32,
    pub _pad: u32,
}

#[map]
static STATS: PerCpuArray<u64> = PerCpuArray::with_max_entries(16, 0);

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(1 << 20, 0);
#[map]
static DNS_EVENTS: RingBuf = RingBuf::with_byte_size(1 << 20, 0);
#[map]
static IP_BLOCKLIST_V4: LpmTrie<[u8; 4], u8> = LpmTrie::with_max_entries(100_000, 0);
#[map]
static IP_BLOCKLIST_V6: LpmTrie<[u8; 16], u8> = LpmTrie::with_max_entries(100_000, 0);

const S_TOTAL: u32 = 0;
const S_IPV4: u32 = 1;
const S_IPV6: u32 = 2;
const S_OTHER_ETH: u32 = 3;
const S_TRUNCATED: u32 = 4;
const S_ABORTED: u32 = 5;
const S_DROPPED_V4: u32 = 6;
const S_DROPPED_V6: u32 = 7;
const S_EXT_ERR: u32 = 8;
const S_ICMPV6: u32 = 9;
const S_FRAGMENT: u32 = 10;
const S_TCP: u32 = 11;
const S_UDP: u32 = 12;
const S_OTHER_L4: u32 = 13;
/// UDP dst_port == 53 的包数（含畸形、含 ringbuf 满丢弃）。
const S_DNS: u32 = 14;

// DnsEvent 字段偏移（与 adblock-common 断言锁死）
const OFF_SRC_IP: usize = 0;
const OFF_DST_IP: usize = 16;
const OFF_SRC_PORT: usize = 32;
const OFF_DST_PORT: usize = 34;
const OFF_QNAME_LEN: usize = 36;
const OFF_IP_VER: usize = 38;
const OFF_PAD: usize = 39;
const OFF_QNAME: usize = 40;
/// 从 DNS 报文起点（含 12B 头）定长拷贝的字节数上限。
const QUESTION_COPY_CAP: usize = 256;

#[xdp]
pub fn adblock_ebpf(ctx: XdpContext) -> u32 {
    match try_adblock(&ctx) {
        Ok(ret) => ret,
        Err(_) => {
            bump(S_ABORTED);
            xdp_action::XDP_ABORTED
        }
    }
}

#[inline(always)]
fn bump(idx: u32) {
    if let Some(v) = STATS.get_ptr_mut(idx) {
        unsafe { *v += 1 };
    }
}

#[derive(Copy, Clone)]
enum L4 {
    Tcp,
    Udp,
    Fragment,
    Icmpv6,
    Other,
}

#[inline(always)]
fn parse_ipv6_ext(
    data: usize,
    data_end: usize,
    mut offset: u32,
    mut nh: u8,
) -> Result<(L4, u32), ()> {
    let mut depth = 0u8;
    while depth < 8 {
        match nh {
            0 | 43 | 60 => {
                let hdr_off = offset as usize;
                if data + hdr_off + 2 > data_end {
                    return Err(());
                }
                let ext_len = unsafe { *((data + hdr_off + 1) as *const u8) } as usize;
                let total_len = (ext_len + 1) * 8;
                if data + hdr_off + total_len > data_end {
                    return Err(());
                }
                nh = unsafe { *((data + hdr_off) as *const u8) };
                offset += total_len as u32;
                depth += 1;
            }
            51 => {
                let hdr_off = offset as usize;
                if data + hdr_off + 2 > data_end {
                    return Err(());
                }
                let pl_len = unsafe { *((data + hdr_off + 1) as *const u8) } as usize;
                let total_len = (pl_len + 2) * 4;
                if data + hdr_off + total_len > data_end {
                    return Err(());
                }
                nh = unsafe { *((data + hdr_off) as *const u8) };
                offset += total_len as u32;
                depth += 1;
            }
            44 => {
                let hdr_off = offset as usize;
                if data + hdr_off + 8 > data_end {
                    return Err(());
                }
                return Ok((L4::Fragment, offset));
            }
            6 => return Ok((L4::Tcp, offset)),
            17 => return Ok((L4::Udp, offset)),
            58 => return Ok((L4::Icmpv6, offset)),
            50 | 59 => return Ok((L4::Other, offset)),
            _ => return Ok((L4::Other, offset)),
        }
    }
    Err(())
}

/// UDP dst_port == 53 时解析 QNAME 并通过 DNS_EVENTS 上报。
/// 所有路径让包继续 pass（Step 3.1 不拦截）。
/// 写入直接落到 ringbuf 槽位（verifier 允许 variable offset 写 ringbuf；不允许写栈）。
#[inline(never)]
fn try_dns(
    data: usize,
    data_end: usize,
    udp_off: usize,
    ip_ver: u8,
    src_ip16: [u8; 16],
    dst_ip16: [u8; 16],
) {
    // 读 UDP 头端口（任一步 Err 一律 return 不 bump：端口未知即不算 dst=53 命中）
    let Ok(src_port) = ptr::read_u16_be(data, udp_off, data_end) else {
        return;
    };
    let Ok(dst_port) = ptr::read_u16_be(data, udp_off + 2, data_end) else {
        return;
    };
    if dst_port != 53 {
        return;
    }

    bump(S_DNS);

    let Ok(udp_len) = ptr::read_u16_be(data, udp_off + 4, data_end) else {
        return;
    };
    let udp_len = udp_len as usize;
    if udp_len < 8 {
        // v4/v6 统一：含 0 一律判畸形（不处理 IPv6 jumbogram）
        return;
    }
    let eff_end = if data + udp_off + udp_len <= data_end {
        data + udp_off + udp_len
    } else {
        // 长度字段与实长不符，按可解析部分处理；Step 3.1 不拦截，3.2 再议
        data_end
    };
    let dns_off = udp_off + 8;

    let Some(mut slot) = DNS_EVENTS.reserve::<DnsEvent>(0) else {
        // ringbuf 满，S_DNS 已计，静默丢
        return;
    };

    // 先解析 —— 失败则 drop slot（discard）
    let raw_len = match dns::copy_payload_into(
        data,
        eff_end,
        dns_off,
        &mut slot,
        OFF_QNAME,
        QUESTION_COPY_CAP,
    ) {
        Ok(n) => n,
        Err(_) => {
            slot.discard(0);
            return;
        }
    };

    // 填固定字段
    let _ = ptr::write_bytes_in(&mut slot, OFF_SRC_IP, &src_ip16);
    let _ = ptr::write_bytes_in(&mut slot, OFF_DST_IP, &dst_ip16);
    let _ = ptr::write_u16_in(&mut slot, OFF_SRC_PORT, src_port);
    let _ = ptr::write_u16_in(&mut slot, OFF_DST_PORT, dst_port);
    let _ = ptr::write_u16_in(&mut slot, OFF_QNAME_LEN, raw_len);
    let _ = ptr::write_u8_in(&mut slot, OFF_IP_VER, ip_ver);
    let _ = ptr::write_u8_in(&mut slot, OFF_PAD, 0);

    slot.submit(0);
}

#[inline(always)]
fn try_adblock(ctx: &XdpContext) -> Result<u32, u32> {
    bump(S_TOTAL);
    let data = ctx.data();
    let data_end = ctx.data_end();

    // 调试：记录最后一个包的线性长度到 STATS[15]
    let pkt_len = (data_end - data) as u64;
    if let Some(v) = STATS.get_ptr_mut(15) {
        unsafe { *v = pkt_len };
    }

    if data + 14 > data_end {
        bump(S_TRUNCATED);
        return Ok(xdp_action::XDP_PASS);
    }

    let eth_proto = unsafe { u16::from_be(*((data + 12) as *const u16)) };

    match eth_proto {
        0x0800 => {
            bump(S_IPV4);
            if data + 34 > data_end {
                bump(S_TRUNCATED);
                return Ok(xdp_action::XDP_PASS);
            }
            let mut src_ip = [0u8; 4];
            unsafe {
                let p = (data + 26) as *const u8;
                let mut i = 0;
                while i < 4 {
                    src_ip[i] = *p.add(i);
                    i += 1;
                }
            }
            if IP_BLOCKLIST_V4.get(Key::new(32, src_ip)).is_some() {
                bump(S_DROPPED_V4);
                return Ok(xdp_action::XDP_DROP);
            }

            let ihl_bytes = match ptr::read_u8(data, 14, data_end) {
                Ok(b) => ((b & 0x0F) as usize) * 4,
                Err(_) => return Ok(xdp_action::XDP_PASS),
            };
            if ihl_bytes < 20 {
                return Ok(xdp_action::XDP_PASS);
            }
            if data + 14 + ihl_bytes > data_end {
                bump(S_TRUNCATED);
                return Ok(xdp_action::XDP_PASS);
            }
            let proto = match ptr::read_u8(data, 23, data_end) {
                Ok(b) => b,
                Err(_) => return Ok(xdp_action::XDP_PASS),
            };
            if proto != 17 {
                return Ok(xdp_action::XDP_PASS);
            }

            let mut dst_ip = [0u8; 4];
            unsafe {
                let p = (data + 30) as *const u8;
                let mut i = 0;
                while i < 4 {
                    dst_ip[i] = *p.add(i);
                    i += 1;
                }
            }

            let mut src16 = [0u8; 16];
            let mut dst16 = [0u8; 16];
            let mut i = 0;
            while i < 4 {
                src16[i] = src_ip[i];
                dst16[i] = dst_ip[i];
                i += 1;
            }

            try_dns(data, data_end, 14 + ihl_bytes, 4, src16, dst16);
        }
        0x86DD => {
            bump(S_IPV6);
            if data + 54 > data_end {
                bump(S_TRUNCATED);
                return Ok(xdp_action::XDP_PASS);
            }

            let mut src_ip = [0u8; 16];
            unsafe {
                let p = (data + 22) as *const u8;
                let mut i = 0;
                while i < 16 {
                    src_ip[i] = *p.add(i);
                    i += 1;
                }
            }
            if IP_BLOCKLIST_V6.get(Key::new(128, src_ip)).is_some() {
                bump(S_DROPPED_V6);
                return Ok(xdp_action::XDP_DROP);
            }

            let next_hdr = unsafe { *((data + 20) as *const u8) };
            match parse_ipv6_ext(data, data_end, 54, next_hdr) {
                Ok((L4::Tcp, _)) => bump(S_TCP),
                Ok((L4::Udp, udp_off)) => {
                    bump(S_UDP);
                    let mut dst_ip = [0u8; 16];
                    unsafe {
                        let p = (data + 38) as *const u8;
                        let mut i = 0;
                        while i < 16 {
                            dst_ip[i] = *p.add(i);
                            i += 1;
                        }
                    }
                    try_dns(data, data_end, udp_off as usize, 6, src_ip, dst_ip);
                }
                Ok((L4::Icmpv6, _)) => {
                    bump(S_ICMPV6);
                    let ev = Event {
                        src_ip,
                        ts_ns: unsafe { aya_ebpf::helpers::bpf_ktime_get_ns() },
                        pkt_len: (data_end - data) as u32,
                        _pad: 0,
                    };
                    let _ = EVENTS.output::<Event>(&ev, 0);
                }
                Ok((L4::Fragment, _)) => bump(S_FRAGMENT),
                Ok((L4::Other, _)) => bump(S_OTHER_L4),
                Err(()) => {
                    bump(S_EXT_ERR);
                    return Ok(xdp_action::XDP_DROP);
                }
            }
        }
        _ => bump(S_OTHER_ETH),
    }
    Ok(xdp_action::XDP_PASS)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = "license")]
#[unsafe(no_mangle)]
static LICENSE: [u8; 13] = *b"Dual MIT/GPL\0";
