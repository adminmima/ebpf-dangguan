#!/usr/bin/env python3
"""
ringbuf05_显式标注output泛型参数Event_v5.py

用途：
    修正 v4 的 E0283 类型推断失败。
    EVENTS.output 的签名是 output<T: ?Sized>(&self, data: impl Borrow<T>, flags: u64)，
    传 &ev 时 T 有多个候选实现，编译无法推断。

测试目标：
    通过 turbofish 显式指定 T=Event，让类型推断成功。

版本变更：
    v4  用 EVENTS.output(&ev, 0)（E0283 类型推断失败）
    v5  用 EVENTS.output::<Event>(&ev, 0)（本脚本）
"""
import sys

PATH = "/root/adblock/adblock-ebpf/src/main.rs"

old = "let _ = EVENTS.output(&ev, 0);"
new = "let _ = EVENTS.output::<Event>(&ev, 0);"

with open(PATH) as f:
    src = f.read()

if "output::<Event>" in src:
    print("已经改过，跳过")
    sys.exit(0)

if old not in src:
    print("ERROR: 未找到 v4 的 output 调用")
    sys.exit(1)

src = src.replace(old, new, 1)
with open(PATH, "w") as f:
    f.write(src)
print("OK: v5 插入成功")
