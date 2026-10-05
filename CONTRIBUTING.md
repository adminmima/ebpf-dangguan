# 贡献指南

## 开发流程

1. 从 master 拉 feature branch：git checkout -b feat/your-feature
2. 开发 + 本地跑 make check 通过
3. Commit message 遵循 Conventional Commits 规范
4. push + 开 PR 到 master

## Commit message 格式

type(scope): subject

| type | 用途 |
|---|---|
| feat | 新功能 |
| fix | bug 修复 |
| refactor | 重构（不改行为） |
| chore | 构建、CI、依赖等杂项 |
| docs | 文档 |
| test | 测试 |
| perf | 性能 |

例子：
- feat(cli): add --verify flag for XDP load validation
- refactor(ebpf): extract ipv6 ext header parser to ipv6.rs
- chore(ci): pin bpf-linker version to 0.11.1

## 环境要求

见 README.md。

## 本地检查

make check     # clippy + fmt + build + test
make verifier  # aya 自身加载验证（需 root + bpffs）

CI 只跑 clippy + fmt + build + test。verifier 需 BPF 特权，仅在本地跑。
