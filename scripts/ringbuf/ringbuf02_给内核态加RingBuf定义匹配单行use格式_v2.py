#!/usr/bin/env python3
"""
ringbuf02_给内核态加RingBuf定义匹配单行use格式_v2.py

用途：
    修正 v1 脚本的 use 块匹配失败问题。实际文件的 use 块是紧凑格式，
    maps 里的 lpm_trie 和 PerCpuArray 在同一行。

测试目标：
    与 v1 相同——插入 Event 结构体和 EVENTS RingBuf map，为事件上报铺路。

思路：
    匹配实际文件的紧凑 use 格式，其余逻辑与 v1 相同。

版本变更：
    v1  预设多行格式，匹配失败
    v2  匹配实际紧凑格式
"""
import sys

PATH = "/root/adblock/adblock-ebpf/src/main.rs"

old_use = """use aya_ebpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::{lpm_trie::{Key, LpmTrie}, PerCpuArray},
    programs::XdpContext,
};"""

new_use = """use aya_ebpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::{lpm_trie::{Key, LpmTrie}, PerCpuArray, RingBuf},
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

if "RingBuf" in src and "EVENTS" in src:
    print("已插入过，跳过")
    sys.exit(0)

if old_use not in src:
    print("ERROR: v2 也没匹配上，需要人工检查 use 块")
    sys.exit(1)
src = src.replace(old_use, new_use, 1)

if old_map not in src:
    print("ERROR: STATS 定义不匹配")
    sys.exit(1)
src = src.replace(old_map, new_map, 1)

with open(PATH, "w") as f:
    f.write(src)
print("OK: v2 插入成功")
