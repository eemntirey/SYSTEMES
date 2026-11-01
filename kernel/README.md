# SYSTEMES kernel — K0

This crate is the first isolated boundary for the future autonomous kernel.

- Target: `x86_64-unknown-none`
- Runtime: `no_std`
- Dependencies: none
- Host: no dependency on Windows, WPF, WebView2, or the .NET native host
- Current scope: a versioned loader-to-kernel handoff contract and pure validation

This is **not yet a bootable kernel binary**. It has no entry point, page tables, interrupt handlers, allocator, drivers, bootloader, or ISO image. Those belong to subsequent K1–K6 milestones in [the roadmap](../docs/BOOTABLE_KERNEL_ISO_ROADMAP.md).

## Local checks

```powershell
rustup target add x86_64-unknown-none
cargo test --manifest-path kernel/Cargo.toml
cargo check --manifest-path kernel/Cargo.toml --target x86_64-unknown-none
```
