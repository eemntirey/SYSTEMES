# SYSTEMES kernel — K0 contract and K1 entry

The kernel is an isolated Rust `no_std` project. It has no dependency on Windows,
WPF, WebView2, or the .NET native host.

- Target: `x86_64-unknown-none`
- Library: versioned `BootInfo` contract and validation
- Binary: `systemes-kernel`, linked at physical address `0x00200000`
- Entry: `_start(BootInfo*)`, x86_64 System V ABI
- Debug trace: QEMU debug port `0xE9`

## K1 status

The binary has been loaded by the GNU-EFI loader and executed after
`ExitBootServices` in QEMU/OVMF. It validates the boot handoff, checks the UEFI
memory-map metadata, discovers the GOP framebuffer and ACPI RSDP, paints a small
framebuffer marker, and emits `SYSTEMES_KERNEL_OK`.

The kernel still inherits the firmware's active address space and CPU execution
environment. It does **not** yet own page tables, GDT/IDT, exception handlers,
interrupt routing, a physical-page allocator, a scheduler, drivers, or user mode.
It is a minimal boot-stage kernel, not a general-purpose OS.

## Local checks

```powershell
rustup target add x86_64-unknown-none
cargo test --manifest-path kernel/Cargo.toml
cargo check --manifest-path kernel/Cargo.toml --target x86_64-unknown-none
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld
```

The reproducible QEMU/OVMF build is defined in `.github/workflows/kernel.yml`.
