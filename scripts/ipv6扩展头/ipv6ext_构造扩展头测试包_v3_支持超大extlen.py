#!/usr/bin/env python3
"""
ipv6ext_构造扩展头测试包.py
用途：构造带 IPv6 扩展头的测试包，用于验证 XDP 的扩展头解析逻辑。
用法：
    sudo python3 ipv6ext_构造扩展头测试包_v3_支持超大extlen.py <模式> <目标地址>
模式：
    hbh        正常的 8 字节 Hop-by-Hop 扩展头 + ICMPv6 Echo
    trunclen   HBH 头声明 ext_len=255（需要 2048 字节），但实际只 8 字节
               用于触发 XDP 扩展头解析的 fail-closed 分支
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

def build_icmpv6_echo(ident, seq, payload=b''):
    return struct.pack('!BBHHH', 128, 0, 0, ident, seq) + payload

def send_pkt(pkt, dst):
    s = socket.socket(socket.AF_INET6, socket.SOCK_RAW, socket.IPPROTO_RAW)
    s.sendto(pkt, (dst, 0, 0, 0))
    s.close()

def test_normal(src_s, dst_s):
    src = ipv6_addr(src_s); dst = ipv6_addr(dst_s)
    icmp = build_icmpv6_echo(0x1234, 1, b'hello')
    csum = icmpv6_checksum(src, dst, icmp)
    icmp = icmp[:2] + struct.pack('!H', csum) + icmp[4:]
    # 正常 HBH：next=58, ext_len=0 (即 8 字节)
    hbh = struct.pack('!BB', 58, 0) + b'\x00' * 6
    pkt = build_ipv6(src, dst, 0, hbh + icmp)
    send_pkt(pkt, dst_s)
    print(f"sent: HBH(8B) + ICMPv6 Echo  -> {dst_s}")

def test_overlong(src_s, dst_s):
    src = ipv6_addr(src_s); dst = ipv6_addr(dst_s)
    icmp = build_icmpv6_echo(0x1234, 1, b'hello')
    csum = icmpv6_checksum(src, dst, icmp)
    icmp = icmp[:2] + struct.pack('!H', csum) + icmp[4:]
    # 超长声明：ext_len=255，要求 2048 字节；但实际只有 8 字节
    hbh = struct.pack('!BB', 58, 255) + b'\x00' * 6
    pkt = build_ipv6(src, dst, 0, hbh + icmp)
    send_pkt(pkt, dst_s)
    print(f"sent: HBH overlong (ext_len=255)  -> {dst_s}")

if __name__ == '__main__':
    if len(sys.argv) < 3:
        print(__doc__); sys.exit(1)
    mode, dst = sys.argv[1], sys.argv[2]
    src = "::1"
    if mode == 'hbh':
        for _ in range(3): test_normal(src, dst)
    elif mode == 'trunclen':
        for _ in range(3): test_overlong(src, dst)
    else:
        print("unknown mode:", mode); sys.exit(1)
