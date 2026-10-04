#!/usr/bin/env python3
"""
ipv6ext_构造扩展头测试包.py
用途：构造带 IPv6 扩展头的测试包，用于验证 XDP 的扩展头解析逻辑。
用法：
    sudo python3 ipv6ext_构造扩展头测试包.py <模式> <目标地址> [接口]
模式：
    hbh    正常的 8 字节 Hop-by-Hop 扩展头 + ICMPv6 Echo
    trunc  截断的 HBH 头（只有 1 字节），触发 fail-closed
接口：
    可选，默认随机端口发送；指定接口需要 root + SO_BINDTODEVICE
"""
import socket, struct, sys

def ipv6_addr(s):
    return socket.inet_pton(socket.AF_INET6, s)

def checksum(data):
    if len(data) % 2:
        data += b'\x00'
    s = 0
    for i in range(0, len(data), 2):
        s += (data[i] << 8) + data[i+1]
    while s >> 16:
        s = (s & 0xffff) + (s >> 16)
    return ~s & 0xffff

def icmpv6_checksum(src, dst, icmp):
    pseudo = src + dst + struct.pack('!I', len(icmp)) + b'\x00\x00\x00\x3a'
    return checksum(pseudo + icmp)

def build_ipv6(src, dst, next_hdr, payload):
    ver_tc_fl = 6 << 28
    return struct.pack('!IHBB', ver_tc_fl, len(payload), next_hdr, 64) + src + dst + payload

def build_hbh(next_hdr, options=b''):
    pad = (8 - ((2 + len(options)) % 8)) % 8
    options += b'\x00' * pad
    hdr_ext_len = ((2 + len(options)) // 8) - 1
    return struct.pack('!BB', next_hdr, hdr_ext_len) + options

def build_icmpv6_echo(ident, seq, payload=b''):
    return struct.pack('!BBHHH', 128, 0, 0, ident, seq) + payload

def send_pkt(pkt, dst):
    s = socket.socket(socket.AF_INET6, socket.SOCK_RAW, socket.IPPROTO_RAW)
    s.sendto(pkt, (dst, 0, 0, 0))
    s.close()

def test_normal(src_s, dst_s):
    src = ipv6_addr(src_s)
    dst = ipv6_addr(dst_s)
    icmp = build_icmpv6_echo(0x1234, 1, b'hello')
    csum = icmpv6_checksum(src, dst, icmp)
    icmp = icmp[:2] + struct.pack('!H', csum) + icmp[4:]
    hbh = build_hbh(58, b'')
    pkt = build_ipv6(src, dst, 0, hbh + icmp)
    send_pkt(pkt, dst_s)
    print(f"sent: HBH(8B) + ICMPv6 Echo  -> {dst_s}")

def test_trunc(src_s, dst_s):
    src = ipv6_addr(src_s)
    dst = ipv6_addr(dst_s)
    bad = b'\x3a'
    pkt = build_ipv6(src, dst, 0, bad)
    send_pkt(pkt, dst_s)
    print(f"sent: truncated HBH (1 byte)  -> {dst_s}")

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print(__doc__)
        sys.exit(1)
    mode, dst = sys.argv[1], sys.argv[2]
    src = "::1"  # 临时用，实际由内核填
    if mode == 'hbh':
        for _ in range(3):
            test_normal(src, dst)
    elif mode == 'trunc':
        for _ in range(3):
            test_trunc(src, dst)
    else:
        print("unknown mode:", mode)
        sys.exit(1)
