#!/usr/bin/env python3
import subprocess, re
out = subprocess.run(['sudo','bpftool','map','dump','name','STATS'],
                     capture_output=True, text=True).stdout
names = ['total','IPv4','IPv6','other_eth','truncated','aborted',
         'dropped_v4','dropped_v6','ext_err','icmpv6','fragment',
         'tcp','udp','other_l4','dns','?15']
idx = -1
for line in out.splitlines():
    if line.startswith('key:'):
        idx += 1; continue
    m = re.search(r'CPU (\d+)\): ([0-9a-f ]+)', line)
    if not m: continue
    cpu, val = m.group(1), m.group(2).strip()
    if val == '00 00 00 00 00 00 00 00': continue
    n = int(''.join(reversed(val.split())), 16)
    print(f'{names[idx]:<12} CPU {cpu}: {n}')
