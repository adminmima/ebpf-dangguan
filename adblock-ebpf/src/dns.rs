//! DNS 报文预处理（Step 3.1）。
//!
//! eBPF 侧只做一件事：从 UDP payload 起点定长拷贝 max_cap 字节到 ringbuf 槽位。
//! DNS 头校验、QNAME 解析、压缩指针判定、根域名判定全部交给用户态。
//!
//! 理由：eBPF verifier 对同一个 packet 多次分散访问的窄化信息不共享，
//! 一旦在函数内同时做"读头部字段"和"循环读 label"，就会触发越界误判。
//! 一次性定长拷贝是 eBPF 侧唯一稳妥的策略。

use core::mem::MaybeUninit;

use crate::ptr::{read_u8, write_u8_in};

/// 从 data + payload_off 起定长拷贝 max_cap 字节到 m[qname_off..]。
/// 返回实际拷贝字节数（报文比 max_cap 短则提前停）。
#[inline(always)]
pub fn copy_payload_into<T>(
    data: usize,
    data_end: usize,
    payload_off: usize,
    m: &mut MaybeUninit<T>,
    qname_off: usize,
    max_cap: usize,
) -> Result<u16, ()> {
    let mut i = 0usize;
    while i < max_cap {
        let b = match read_u8(data, payload_off + i, data_end) {
            Ok(b) => b,
            Err(_) => return Ok(i as u16),
        };
        write_u8_in(m, qname_off + i, b)?;
        i += 1;
    }
    Ok(max_cap as u16)
}
