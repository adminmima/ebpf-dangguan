#!/usr/bin/env python3
"""
ringbuf03_在ICMPv6分支里写事件到RingBuf_v3.py

用途：
    在 try_adblock 的 IPv6 处理路径里，当识别到 L4::Icmpv6 时，
    通过 EVENTS.reserve + write + submit 向用户态上报一条 Event。

测试目标：
    RingBuf 内核侧写入通路。用户态还没接，先确认编译通过、加载正常、
    bpftool map dump EVENTS 看到数据被消费或堆积。

思路：
    aya-ebpf 的 RingBuf API：
        if let Some(mut entry) = EVENTS.reserve::<Event>(0) {
            entry.src_ip = ...;    // 通过 DerefMut
            entry.submit(0);
        }
    reserve 返回 None 表示 RingBuf 满——本次丢弃，不阻塞。
    timestamp 用 aya_ebpf::helpers::bpf_ktime_get_ns()。

版本变更：
    v1  插入 use + Event + EVENTS（匹配失败）
    v2  修正匹配格式（成功）
    v3  本脚本：在 Icmpv6 分支写入事件
"""
import sys

PATH = "/root/adblock/adblock-ebpf/src/main.rs"

# 需要 src_ip 在 Icmpv6 分支可见——确认它在 try_adblock 的 IPv6 分支里
old = """                Ok(L4::Icmpv6)    => bump(S_ICMPV6),"""

new = """                Ok(L4::Icmpv6)    => {
                    bump(S_ICMPV6);
                    if let Some(mut entry) = EVENTS.reserve::<Event>(0) {
                        entry.src_ip = src_ip;
                        entry.ts_ns = unsafe { aya_ebpf::helpers::bpf_ktime_get_ns() };
                        entry.pkt_len = (data_end - data) as u32;
                        entry._pad = 0;
                        entry.submit(0);
                    }
                },"""

with open(PATH) as f:
    src = f.read()

if "EVENTS.reserve::<Event>" in src:
    print("已经改过，跳过")
    sys.exit(0)

if old not in src:
    print("ERROR: 未找到 Icmpv6 分支，检查实际格式")
    sys.exit(1)

src = src.replace(old, new, 1)

with open(PATH, "w") as f:
    f.write(src)
print("OK: v3 插入成功")
