.PHONY: help build ebpf verifier clippy clippy-host clippy-ebpf fmt test check clean

help:
	@echo "make check     - clippy + fmt + build + test + verifier"
	@echo "make build     - build workspace (ebpf via aya-build)"
	@echo "make ebpf      - build ebpf to target/ebpf-check"
	@echo "make verifier  - aya self-load verify (root + /sys/fs/bpf)"
	@echo "make clippy       - clippy-host + clippy-ebpf"
	@echo "make clippy-host  - clippy workspace, 排除 adblock-ebpf"
	@echo "make clippy-ebpf  - clippy adblock-ebpf, bpfel target + build-std"
	@echo "make fmt       - cargo fmt --check"
	@echo "make test      - cargo test"
	@echo "make clean     - cargo clean"

build:
	cargo build --release

ebpf:
	cargo build -p adblock-ebpf --target bpfel-unknown-none --release -Z build-std=core --target-dir target/ebpf-check

verifier: build
	@if [ "$$(id -u)" != "0" ]; then echo "verifier needs root (XDP attach)"; exit 1; fi
	@if ! mountpoint -q /sys/fs/bpf; then echo "/sys/fs/bpf not mounted: mount -t bpf bpf /sys/fs/bpf"; exit 1; fi
	@echo "用 aya 自身加载验证（bpftool/libbpf 不支持 aya 标准 #[map] 格式）..."
	@timeout 30 cargo run --release -p adblock -- --iface lo --verify; \
	RET=$$?; \
	if [ $$RET -eq 0 ]; then \
		echo "verifier OK"; \
	elif [ $$RET -eq 124 ]; then \
		echo "verifier FAILED: timeout (30s)"; exit 1; \
	else \
		echo "verifier FAILED: exit code $$RET"; exit 1; \
	fi

clippy: clippy-host clippy-ebpf

clippy-host:
	cargo clippy --workspace --exclude adblock-ebpf --all-targets -- -D warnings

clippy-ebpf:
	cargo clippy -p adblock-ebpf --release --target bpfel-unknown-none \
		-Z build-std=core --target-dir target/ebpf-check -- -D warnings

fmt:
	cargo fmt --all -- --check

test:
	cargo test --workspace

check: clippy fmt build test verifier

clean:
	cargo clean
	rm -rf target/ebpf-check
