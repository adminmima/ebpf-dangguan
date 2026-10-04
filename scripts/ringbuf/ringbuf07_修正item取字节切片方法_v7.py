#!/usr/bin/env python3
"""
ringbuf07_修正item取字节切片方法_v7.py

用途：
    修正 v6 的 `item.as_slice()` 调用。
    `as_slice` 是 str 的方法（在 Rust 1.9x 阶段还可能涉及 unstable
    str_as_str feature），而 item 实际是 [u8] 类型。
    改用 `item.as_ref()` 得到 &[u8]。

测试目标：
    与 v6 相同，让 RingBuf 消费代码编译通过。

版本变更：
    v6  用 item.as_slice()（编译失败，str 方法冲突）
    v7  改用 item.as_ref()（本脚本）
"""
import sys

PATH = "/root/adblock/adblock/src/main.rs"

with open(PATH) as f:
    src = f.read()

if "item.as_slice()" not in src:
    print("已经改过或格式不匹配")
    sys.exit(1)

src = src.replace("let data = item.as_slice();", "let data: &[u8] = item.as_ref();", 1)

with open(PATH, "w") as f:
    f.write(src)
print("OK: v7 修正成功")
