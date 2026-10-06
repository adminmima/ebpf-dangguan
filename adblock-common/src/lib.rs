#![no_std]

/// DNS QNAME 上报事件（内核 ↔ 用户态 ABI，两侧逐字节一致）。
///
/// 本结构不得引入任何带 Drop 的字段类型。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct DnsEvent {
    pub src_ip: [u8; 16],
    pub dst_ip: [u8; 16],
    pub src_port: u16,
    pub dst_port: u16,
    pub qname_len: u16,
    pub ip_ver: u8,
    pub _pad: u8,
    pub qname: [u8; 256],
}

const _: () = assert!(core::mem::size_of::<DnsEvent>() == 296);
const _: () = assert!(core::mem::offset_of!(DnsEvent, src_ip) == 0);
const _: () = assert!(core::mem::offset_of!(DnsEvent, dst_ip) == 16);
const _: () = assert!(core::mem::offset_of!(DnsEvent, src_port) == 32);
const _: () = assert!(core::mem::offset_of!(DnsEvent, dst_port) == 34);
const _: () = assert!(core::mem::offset_of!(DnsEvent, qname_len) == 36);
const _: () = assert!(core::mem::offset_of!(DnsEvent, ip_ver) == 38);
const _: () = assert!(core::mem::offset_of!(DnsEvent, _pad) == 39);
const _: () = assert!(core::mem::offset_of!(DnsEvent, qname) == 40);
