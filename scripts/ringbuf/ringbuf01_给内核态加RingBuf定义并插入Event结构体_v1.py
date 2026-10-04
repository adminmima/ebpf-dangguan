#!/usr/bin/env python3
"""
ringbuf01_给内核态加RingBuf定义并插入Event结构体_v1.py

用途：
    修改 adblock-ebpf/src/main.rs，加入 RingBuf 事件上报所需的两部分：
    1) use 块里引入 RingBuf 类型，定义 Event 结构体
    2) 在 STATS 后新增 EVENTS RingBuf map（1 MiB）

测试目标：
    RingBuf 通路的最小验证——内核态能 reserve+submit 事件，用户态能消费。
    本步只做"数据结构插入"，不做实际写入逻辑。

思路：
    Event 结构体：
        src_ip: [u8; 16]   — IPv6 源地址
        ts_ns: u64         — 时间戳（纳秒）
        pkt_len: u32       — 包长
        _pad: u32          — 8 字节对齐填充
    总大小 32 字节，内核和用户态共享这个 #[repr(C)] 定义。

版本变更：
    v1  首次建立，只插入 use + map 定义，不写逻辑
"""
import sys

PATH = "/root/adblock/adblock-ebpf/src/main.rs"

old_use = """use aya_ebpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::{
        lpm_trie::{Key, LpmTrie},
        PerCpuArray,
    },
    programs::XdpContext,
};"""

new_use = """use aya_ebpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::{
        lpm_trie::{Key, LpmTrie},
        PerCpuArray,
        RingBuf,
    },
    programs::XdpContext,
};

#[repr(C)]
pub struct Event {
    pub src_ip: [u8; 16],
    pub ts_ns: u64,
    pub pkt_len: u32,
    pub _pad: u32,
}"""

old_map = """#[map]
static STATS: PerCpuArray<u64> = PerCpuArray::with_max_entries(16, 0);"""

new_map = """#[map]
static STATS: PerCpuArray<u64> = PerCpuArray::with_max_entries(16, 0);

#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(1 << 20, 0);  // 1 MiB"""

with open(PATH) as f:
    src = f.read()

if old_use not in src:
    print("ERROR: use 块不匹配，可能已经改过"); sys.exit(1)
src = src.replace(old_use, new_use, 1)

if old_map not in src:
    print("ERROR: STATS 定义不匹配"); sys.exit(1)
src = src.replace(old_map, new_map, 1)

with open(PATH, "w") as f:
    f.write(src)
print("OK: use + Event + EVENTS map 插入成功")
