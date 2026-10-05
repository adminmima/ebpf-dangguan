# 交接文档：adblock-ebpf 项目

> 归档说明：本文档基于 2026-10-05 会话实际执行的命令、git 输出、讨论结论整理。
> 第一部分的 git 数据在归档前已重跑核对（HEAD: b2a3aba）。

## 一、项目现状

### 1.1 分支与 commit

核对命令：

    cd /root/adblock && git log --oneline && git status --short && git branch -v

master 上共 18 个 commit（4 基础 + 3 骨架 + 6 feature + 1 merge + 3 清理 + 1 merge）。
分支：只有 master，工作区干净。

完整 commit 列表（从旧到新）：

- ceb3793 milestone: LPM Trie + IPv6 ext header + fail-closed + RingBuf all verified
- 4b30ed9 chore: remove backup files from tracking, tighten gitignore
- ed2890d user: remove AYA_LOGS init, RFC 5952 IPv6 formatting
- 7a73049 chore(repo): add Makefile, CI workflow, toolchain pin, env requirements
- e8d7420 style: apply rustfmt to ebpf and user main.rs
- 9900922 chore(repo): add CONTRIBUTING, CHANGELOG, PR template
- e44fbec merge: chore/repo-scaffolding into master
- acb2983 feat: add --verify flag for aya self-load verification
- e6a1889 ci: split clippy targets into host and ebpf

- ebc78e5 fix: resolve clippy errors (L4 dead_code, needless_range_loop, Key borrow)
- 85eebd8 chore(deps): remove unused aya-log, tighten gitignore, add dep comments
- a2eb1d7 build: verifier uses aya self-load instead of bpftool
- e9ee6b4 fix(build): exclude adblock-ebpf from cargo test
- 9d77d60 merge: feat/verify-flag into master
- 1f48f54 chore(repo): 归档草稿与快照脚本，dev-tools 收纳运行时工具
- 65b5b09 chore(license): 用户态 AGPL-3.0-or-later，eBPF Dual MIT/GPL
- 06aa6b3 chore(release): 发布准备收尾
- b2a3aba merge: release-prep into master (发布前清理与 license 改造)

当前状态核对：sudo make check 2>&1 | tail -15（现场执行为准）。

### 1.2 环境版本

- 内核：7.2.8-arch1-2（uname -r）
- Rust toolchain：nightly-2026-09-29 (1.101.0-nightly, c1070d693)，见 rust-toolchain.toml
- bpf-linker：0.11.1（bpf-linker --version）
- bpftool：v7.8.0，libbpf v1.8（bpftool version）
- aya：0.14.0 / aya-ebpf：0.2.1 / aya-obj：0.3.0 / aya-log-ebpf：0.2.0（Cargo.lock）
- 最低内核要求：5.8（RingBuf 硬要求）

## 二、已完成能力清单

- 三 crate workspace 骨架（common / ebpf / user）：ceb3793
- XDP 以太头解析 + IPv4/IPv6 区分 + PerCpuArray 计数器：ceb3793
- LPM Trie IPv4 黑名单（[u8; 4] key + prefix_len 显式）：ceb3793
- LPM Trie IPv6 黑名单（[u8; 16] key）：ceb3793
- IPv6 扩展头解析（HBH / Routing / DestOpts / AH / Fragment）：ceb3793
- fail-closed：扩展头解析失败 → XDP_DROP + 计数：ceb3793
- RingBuf 内核→用户态事件通路（tokio AsyncFd 异步消费）：ceb3793
- RFC 5952 风格 IPv6 格式化（::1 而非 0:0:0:0:0:0:0:1）：ed2890d
- 移除无用 AYA_LOGS 初始化：ed2890d
- rustfmt 全应用：e8d7420
- Makefile（build / ebpf / verifier / clippy-host / clippy-ebpf / fmt / test / check）：7a73049 + a2eb1d7 + e9ee6b4
- GitHub Actions CI（clippy 双 target + build + test）：7a73049 + e6a1889 + e9ee6b4
- rust-toolchain.toml 钉 nightly-2026-09-29：7a73049
- README 环境要求段：7a73049
- CONTRIBUTING / CHANGELOG / PR template：9900922

- --verify 短路模式（load → attach → map 检查 → detach → exit）：acb2983
- verifier 改 aya 自加载（放弃 bpftool）：a2eb1d7
- clippy 双侧拆分：e6a1889
- clippy 全绿（L4 去字段 / needless_range_loop / Key 引用）：ebc78e5
- 移除用户态 unused aya-log：85eebd8
- cargo test --exclude adblock-ebpf：e9ee6b4

## 三、踩坑与定案记录（最重要）

### 3.1 验证路径

❌ 否决：bpftool/libbpf 加载 aya 产物
- 现象：bpftool prog load 报 legacy map definitions in 'maps' section are not supported by libbpf v1.0+
- 根因：aya 标准 #[map] 宏生成的 BTF map 格式与 libbpf 不兼容（aya-rs/aya Issue #913，2024-03 提出，仍 Open）。PR #1457 只修了 Array / RingBuf / SkStorage 三条路径，LpmTrie 未涉及
- 定案：放弃 bpftool 路径，verifier 改 aya 自身加载

❌ 否决：迁移到 aya_ebpf::btf_maps API
- 支线评估结果：btf_maps 有 PerCpuArray / RingBuf，无 LpmTrie
- 项目核心依赖 LpmTrie（IPv4/IPv6 前缀匹配），迁移不可行
- 定案：放弃支线，保留标准 #[map] 宏

❌ 否决：bpftool --mapcompat flag 绕过
- 只让 bpftool 不报错，内核侧 map 创建仍可能失败
- 且不验证 aya 实际加载路径
- 定案：不采用

❌ 修正：aya-log 验证方式
- 一度误以为日志走 /sys/kernel/debug/tracing/trace_pipe
- 实际：aya-log-ebpf 走 perf_event_array，由用户态 EbpfLogger::init 消费后交给标准 log 输出，日志出现在用户态程序终端
- trace_pipe 只显示 bpf_printk
- 两个机制可并存

### 3.2 依赖与 lints

❌ 否决：workspace lints 全局关 unused_dependencies
- 三次提出，三次否决
- 理由：全局 allow 会掩护未来真正多余的依赖
- 定案：不采用；adblock-common / adblock-ebpf 的 unused 警告用注释说明（build.rs 通过 cargo_metadata 隐式引用）

❌ 否决：保留 unused aya-log 用户态依赖
- 定案：移除（85eebd8）；Step 3.1 需要时 cargo add aya-log 加回（十秒的事）
- 保留 aya-log-ebpf（eBPF 侧）不动——它是 Step 3.1 trace_pipe 调试要用的

❌ 否决：bpf-linker --version 不 pin
- CI 里 cargo install bpf-linker --locked 只锁依赖树，不锁 bpf-linker 自身版本
- 定案：CI 和 README 都钉 --version 0.11.1（从 bpf-linker --version 实测取）

❌ 修正：版本锁定认知
- Cargo.lock 覆盖不到三样东西：bpf-linker（全局 cargo install）、nightly 工具链、内核最低版本
- 定案：rust-toolchain.toml 钉日期 + targets + components；README 声明内核 ≥ 5.8 + bpf-linker 版本

### 3.3 eBPF 编程约定

✅ 保留：IPv6 扩展头 while depth < 8 有界循环
- 一度被批评"禁止 while 循环"——原话是"循环必须有编译期常量上界"，不是禁 while
- 有界循环 kernel 5.3+ 支持；RingBuf 要求 5.8+，本项目最低内核 ≥ 5.8
- 隐性成本：循环迭代 × 分支数会导致 verifier 状态爆炸。depth < 8 安全，上界不能随手写

✅ 保留：ptr_at 封装（选"现在做"，非"第二次 bug 触发"）
- 理由：eBPF bug 单价远高于普通 Rust；DNS QNAME 解析是边界检查最密集的场景
- 先铺路比 Step 3.1 边写边补省事

✅ 定案：fail-closed 原则
- IPv6 扩展头解析失败（越界 / 深度超限 / 分片头数据不够）→ XDP_DROP + 计数器
- 放行例外：ICMPv6（Android SLAAC 必需）、Fragment（源 IP 已查黑名单）、ESP、No Next Header、未知协议
- 验证方式：注入 return Err(()) → ext_err 涨、icmpv6 不涨 → 撤销

✅ 定案：unwrap 策略
- 启动路径：改 expect("上下文")，fail-fast
- 事件循环 / 热更新路径：消灭 unwrap（panic 杀死常驻服务）

✅ 定案：LPM Trie 用法
- Key::new(prefix_len, addr_bytes)，prefix_len 必须显式携带（IPv4=32，IPv6=128）
- IPv4 用 [u8; 4] 而非 u32（字节序：网络序 vs 主机序在前缀匹配上语义不同）
- LpmTrie::get 接受 impl Borrow，&Key::new(...) 被 clippy 判 needless_borrow

✅ 定案：eBPF 侧栈使用
- 每函数帧独立 512 字节（kernel 4.16+ BPF-to-BPF 调用，最多 32 帧）
- 编码守则：单函数栈 < 512 字节，大数组用 PerCpuArray 中转

✅ 定案：内核态禁止字符串比较
- hot path 只比 hash（如前 8 字节）

### 3.4 工具链与流程

❌ 否决：频繁 cargo clean
- 用户明确：不是必须删的就不删，确实需要时才删
- 定案：优先 touch Cargo.toml → cargo clean -p → 全清（最后手段）
- 本次全清是因为 workspace manifest 层 stale 缓存

❌ 踩坑：commit message 里的感叹号
- ![no_main] 在双引号内触发 bash 历史扩展，报 event not found
- 定案：带感叹号的 message 用单引号包或 heredoc

❌ 修正：CI 价值判断
- 不接受"等 3+ 协作者再搞 CI"论
- eBPF 项目 verifier 行为随内核版本漂移，回归风险高，CI 必做
- 但 CI 不跑 verifier（GitHub Actions 默认 runner 无 BPF 权限），verifier 只在本地 sudo make verifier

❌ 修正：cargo test 目标
- cargo test --workspace 会尝试编译 adblock-ebpf 为 host test，因 no_main 报 undefined symbol: main
- 定案：cargo test --workspace --exclude adblock-ebpf

✅ 定案：build.rs warning 是噪音
- adblock@0.1.0: Compiling ... 那一大坨是 aya-build 在 build.rs 里编译 eBPF 的输出，不是用户态 clippy 的结果
- 后续不再报告这类噪音

### 3.5 功能设计

✅ 定案：DNS 拦截用 XDP_DROP（非 NXDOMAIN）
- 丢包 CPU 开销最低（一次函数返回）；NXDOMAIN 需 bpf_xdp_adjust_head 改包头 + 构造响应 + 重算校验和，CPU 约 3-5 倍
- 客户端体验：丢包会超时等待（默认 5 秒）；NXDOMAIN 立刻返回
- 定案：Step 3.1 先做丢包，NXDOMAIN 留 Step 3.4 作为可选模式

✅ 定案：--verify 短路模式
- 流程：加载 → XDP attach → map 创建验证 → detach → exit(0)
- 全程同步，禁止后台轮询
- 任何一步失败输出 stderr 并非零退出
- 用 program.detach(link_id)（aya 0.14 define_link_wrapper 生成），非旧版全局 detach API

## 四、既定规矩（不得重新讨论）

### 4.1 git 纪律

1. Conventional Commits 格式：(type)(scope): subject
2. feature branch 流程：git checkout -b feat/xxx → 开发 → --no-ff merge 回 master → 删分支
3. 禁止 git add -A / git add .：暂存前对照 git status 只挑目标文件
4. 禁止把不同性质的改动混进一个 commit
5. commit message 带感叹号用单引号或 heredoc
6. 物理备份文件进 .gitignore（*.pre_v*、*.bak、*.normal、*.working、*_bak）

### 4.2 文件与依赖

7. 默认不删文件、不清缓存、不删依赖；确实需要删时先确认再动
8. cargo clean 是最后手段（先 touch → cargo clean -p → 全清）
9. 中间过程脚本保留版本，不覆盖（scripts/<功能>/<动作>_<中文说明>_v<N>.py）
10. 开发脚本放 /scripts/，不进 git（.gitignore 已排除）
11. 构建时依赖容忍度（bpf-linker 88 依赖可接受）；运行时不加冗余依赖

### 4.3 代码

12. fail-closed：解析失败 → DROP + 计数，不放行
13. 边界检查必过 ptr_at
14. 内核态禁止字符串比较
15. 内核态禁止 String / Vec / format!
16. 内核态栈 < 512 字节 / 单函数
17. 所有包头字段用 from_be / from_be_bytes 转主机序
18. 循环必须有编译期常量上界
19. 启动路径 expect("上下文")；事件循环/热更新路径消灭 unwrap
20. LPM Trie Key::new(prefix_len, ...)，prefix_len 显式

### 4.4 版本与工具链

21. aya / aya-ebpf / aya-log / aya-log-ebpf 必须同一 release 线
22. 升级 bpf-linker 前跑 make check
23. 内核 ≥ 5.8（RingBuf 硬要求）

### 4.5 验证

24. make check 是提交前必过项
25. verifier 只在本地跑（sudo make verifier）
26. CI 不跑 verifier（runner 无 BPF 权限）

## 五、待办与下一步：Step 3.1

### 5.1 范围

做：
- XDP 抓 UDP 且 dst port 53（IPv4 + IPv6 双栈）
- 解析 DNS 报文头（12 字节固定头）
- 解析 Question section：QNAME（长度前缀标签序列）
- QNAME 提取为字节数组
- 通过 RingBuf 上报（扩展现有 Event 结构体或新增）
- 用户态接收并打印域名

不做（留给后续步骤）：
- 不查黑名单、不拦截
- 不处理 TCP DNS
- 不处理 DNS 压缩指针（QNAME 一般不用，但代码要防御）
- 不处理 DoH / DoT
- 不处理 DNS 响应（A/AAAA 记录）

### 5.2 三个定案

- 拦截策略：Step 3.1 不做拦截；Step 3.2 做拦截时用 XDP_DROP（非 NXDOMAIN）；NXDOMAIN 留 Step 3.4
- trace_pipe 启用：启用。保留的 aya-log-ebpf 依赖用于内核态 info!，日志走用户态终端（非 trace_pipe）。DNS 解析调试用它比 RingBuf 更顺手
- 测试域名清单：正常域名 baidu.com、qq.com；裸黑名单 ads.example.com；多级子域 a.b.doubleclick.net（为 Step 3.2 后缀匹配备料）

### 5.3 预期新建文件

adblock-ebpf/src/ 下新增：
- dns.rs：DNS 报文解析模块
- ptr.rs：边界访问原语（ptr_at 等）
- maps.rs：map 定义集中（从 main.rs 抽出）
- main.rs 修改：加 UDP dst 53 分支 + 调用 dns.rs

具体方案在新会话中给出，本会话不预设实现细节。

### 5.4 入口命令（新会话第一件事）

    cd /root/adblock
    git log --oneline -3
    git status --short
    sudo make check 2>&1 | tail -5

若三项输出与本文件第一部分一致，直接进入 Step 3.1。

---

## 六、仓库清理与发布准备记录

执行时间：2026-10-05
触发分支：release-prep（已合并回 master，merge commit b2a3aba）

### 6.1 发布主目录确定

/root/adblock 为唯一发布目录。发布物 = git 追踪的文件。所有未跟踪文件、本地调试工具、中间产物不随发布。

### 6.2 归档明细

全部移入 zawujian_杂物间/（.gitignore 罩住永不入库）：

- 草稿/：5 个文件 —— main.rs.bak、main.rs.normal、main.rs.working、main.rs.v5_bak、main.rs.pre_v8
- scripts快照/：10 个文件 —— ringbuf01-07 七个脚本、ipv6ext v2、user01、fix03
- captures/：2 个文件 —— hbh_test.pcap、trunc_test.pcap

物理保留，随时可取回。git 历史中 scripts/ 与 main.rs.pre_v8 的移除可追溯（相关历史在 1f48f54 前的 commit 中仍完整）。

### 6.3 dev-tools/ 新增

两个具备长期复用价值的运行时工具从 scripts/ 移入 dev-tools/，保留在发布物中：

- counters_查看各计数器非零值_v1.py：读 STATS map，只打印非零项
- ipv6ext_构造扩展头测试包_v3_支持超大extlen.py：构造 IPv6 HBH 包，用于扩展头解析的回归测试

配 dev-tools/README.md 说明用途。

### 6.4 解除 git 追踪

- adblock/src/main.rs.pre_v8：git rm --cached，物理移入杂物间
- scripts/ 12 个文件：git rm -r --cached scripts/，2 个 git mv 到 dev-tools，其余物理移入杂物间

### 6.5 license 改造终案

- 用户态（adblock、adblock-common）：AGPL-3.0-or-later（根 Cargo.toml 声明，两个 crate license.workspace = true 继承）
- eBPF 内核态（adblock-ebpf）：Dual MIT/GPL（Cargo.toml 显式字段；ELF license section 字符串保持 "Dual MIT/GPL\0" 未动，内核 bpf_prog_load 校验依赖它）
- 新增 LICENSE：AGPL-3.0 全文（661 行）
- 新增 NOTICE：说明项目骨架源自 aya-rs/aya-template（MIT OR Apache-2.0），用户态 / eBPF 侧授权差异
- 保留：LICENSE-MIT、LICENSE-APACHE、LICENSE-GPL2

README 的 License 段同步，明确区分用户态 AGPL、eBPF Dual MIT/GPL、骨架来源。

依据：eBPF 程序使用 GPL-only helper（bpf_ktime_get_ns），内核在 bpf_prog_load 时校验 ELF license section 字符串，非 GPL 兼容字符串会导致 helper 被拒。

### 6.6 .cargo/config.toml runner 注释

runner = "sudo -E" 加注释说明用途——XDP attach 需要 root，cargo run / cargo test 自动提权，clone 后无需手动 sudo。

### 6.7 CHANGELOG 冻结

[Unreleased] → [0.1.0] - 2026-10-05，首次发布从 0.1.0 起步。原 [0.1.0] - 2026-10-04 段（骨架那批）的内容合并到冻结版本。

### 6.8 make check 状态

发布准备 commit（06aa6b3）后跑 sudo make check 全绿：clippy-host、clippy-ebpf、fmt、build、test 全通过，verifier 7 项 OK。

### 6.9 收尾状态

- master HEAD：b2a3aba（merge: release-prep into master）
- 分支：仅 master
- 工作区：干净
- 未执行的操作：git remote add + git push（发布动作，待 GitHub 仓库建立后执行）

文档结束。

---

## 附录：发布后 CI 修复记录

时间：2026-10-06（首次 push 后）
触发：首次 push 后 CI 失败，逐项定位修复。共 3 个 commit。

### 修复 1：0f7959f — 移除 bpfel-unknown-none targets

问题：dtolnay/rust-toolchain 的 targets 参数触发 rustup target add
bpfel-unknown-none，而 nightly 无此 target 的预编译 rust-std，报
component 'rust-std' for target 'bpfel-unknown-none' is unavailable。

定案：删掉 targets 参数。eBPF 编译通过 -Z build-std=core 从
rust-src 源码构建 core，不需要预编译的 target 库。

### 修复 2：225de80 — bpf-linker 改用预编译二进制

问题：CI 里 cargo install bpf-linker 报 "could not find llvm-config in
directories specified by environment variable PATH"。bpf-linker 的
build.rs 需要 LLVM 开发库（llvm-sys 链接 libLLVM），Ubuntu runner
默认没有，编译直接失败。

本地能过是因为 Arch 系统自带 LLVM 23.1.1（llvm-config --version 输出
23.1.1），CI 的 Ubuntu 环境没有。

定案：改用官方 GitHub Release 提供的 x86_64-unknown-linux-musl 静态
二进制，不依赖系统 LLVM：

    curl -sSL -o bpf-linker.tar.zst \
      https://github.com/aya-rs/bpf-linker/releases/download/v0.11.1/bpf-linker-x86_64-unknown-linux-musl.tar.zst
    tar -I zstd -xf bpf-linker.tar.zst -C /usr/local/bin

（apt 需先装 zstd 解压）。约几秒完成，比 cargo install 快一个数量级。

### 修复 3：cae2660 — aya-build 显式指定钉版工具链

问题：CI 到 build 步报 "toolchain 'nightly-x86_64-unknown-linux-gnu'
is not installed"。根因：aya-build 的 Toolchain::default() 返回
Toolchain::Nightly，as_str() 硬编码字符串 "nightly"，CI 只装了钉版
nightly-2026-09-29，无 "nightly" 别名。

本地能过是因为本地 rustup 默认 toolchain 就是 "nightly"。

定案：adblock/build.rs 改 Toolchain::default() 为
Toolchain::Custom(EBPF_TOOLCHAIN)，其中

    const EBPF_TOOLCHAIN: &str = "nightly-2026-09-29";

与 rust-toolchain.toml 的 channel 一致，升级时两处同步改。

### 最终 CI 状态

run id 37339332119 全绿，所有 step success：
checkout、rust-toolchain、Install bpf-linker (prebuilt)、fmt、
clippy (host)、clippy (ebpf)、build、test。

verifier 不在 CI 里跑（runner 无 BPF 权限），仅本地 sudo make verifier。

### 收尾

master HEAD：cae2660
origin/master 已同步
工作区干净
