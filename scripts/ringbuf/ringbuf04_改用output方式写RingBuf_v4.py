#!/usr/bin/env python3
"""
ringbuf04_改用output方式写RingBuf_v4.py

用途：
    修正 v3 的 RingBuf 写入方式。v3 用 `entry.src_ip = ...` 直接给
    RingBufEntry 字段赋值，但 aya-ebpf 0.2.1 里 RingBufEntry<T> 的
    DerefMut 目标是 MaybeUninit<T> 而非 T，字段访问失败。

测试目标：
    RingBuf 内核侧写入通路，改用更简单的 RingBuf::output() API。

思路：
    output<T>(&self, data: impl Borrow<T>, flags: u64) -> Result<(), i32>
    直接传 &Event，内核会把数据复制进 RingBuf。比 reserve 少一步 submit，
    也不用处理 MaybeUninit 的借用问题。

版本变更：
    v1  插入 use + Event + EVENTS（匹配失败）
    v2  修正 use 匹配格式（成功）
    v3  用 entry.src_ip = ... 写（编译失败，字段不存在）
    v4  改用 EVENTS.output(&ev, 0)（本脚本）
"""
import sys

PATH = "/root/adblock/adblock-ebpf/src/main.rs"

old = """                Ok(L4::Icmpv6)    => {
                    bump(S_ICMPV6);
                    if let Some(mut entry) = EVENTS.reserve::<Event>(0) {
                        entry.src_ip = src_ip;
                        entry.ts_ns = unsafe { aya_ebpf::helpers::bpf_ktime_get_ns() };
                        entry.pkt_len = (data_end - data) as u32;
                        entry._pad = 0;
                        entry.submit(0);
                    }
                },"""

new = """                Ok(L4::Icmpv6)    => {
                    bump(S_ICMPV6);
                    let ev = Event {
                        src_ip,
                        ts_ns: unsafe { aya_ebpf::helpers::bpf_ktime_get_ns() },
                        pkt_len: (data_end - data) as u32,
                        _pad: 0,
                    };
                    let _ = EVENTS.output(&ev, 0);
                },"""

with open(PATH) as f:
    src = f.read()

if "EVENTS.output(&ev" in src:
    print("已经改过，跳过")
    sys.exit(0)

if old not in src:
    print("ERROR: 未找到 v3 的代码块")
    sys.exit(1)

src = src.replace(old, new, 1)

with open(PATH, "w") as f:
    f.write(src)
print("OK: v4 插入成功")
