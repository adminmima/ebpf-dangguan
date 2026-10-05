use anyhow::{Context as _, anyhow};
use aya_build::Toolchain;

// 与 rust-toolchain.toml 的 channel 保持一致；升级时两处同步改。
const EBPF_TOOLCHAIN: &str = "nightly-2026-09-29";

fn main() -> anyhow::Result<()> {
    let cargo_metadata::Metadata { packages, .. } = cargo_metadata::MetadataCommand::new()
        .no_deps()
        .exec()
        .context("MetadataCommand::exec")?;
    let ebpf_package = packages
        .into_iter()
        .find(|cargo_metadata::Package { name, .. }| name.as_str() == "adblock-ebpf")
        .ok_or_else(|| anyhow!("adblock-ebpf package not found"))?;
    let cargo_metadata::Package {
        name,
        manifest_path,
        ..
    } = ebpf_package;
    let ebpf_package = aya_build::Package {
        name: name.as_str(),
        root_dir: manifest_path
            .parent()
            .ok_or_else(|| anyhow!("no parent for {manifest_path}"))?
            .as_str(),
        ..Default::default()
    };
    aya_build::build_ebpf([ebpf_package], Toolchain::Custom(EBPF_TOOLCHAIN))
}
