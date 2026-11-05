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

The kernel now owns four-level identity page tables, W^X section mappings,
GDT/IDT exception and IRQ gates, TSS/IST stacks, a physical-frame allocator,
ACPI/MADT discovery, xAPIC/IOAPIC routing, and PS/2 input paths. K2.7 adds the
first single-core preemptive scheduler using a periodic Local APIC timer,
per-task kernel stacks, saved interrupt contexts, round-robin task selection,
blocked-task wake-up, and an IRQ-safe mutex. K2.8 adds explicit x87/SSE
ownership (CR0/CR4) and a 512-byte FXSAVE64 image per task so XMM, MXCSR and
x87 state survive preemptive context switches.

This remains an early autonomous kernel, not yet a general-purpose OS. Tasks
still execute in ring 0 and share one address space. SMP, calibrated timekeeping,
AVX/AVX-512 upper-state switching (XSAVE/XCR0), user mode, system calls, a VFS,
storage drivers, and a complete device model remain future work. See
[the K2.7 scheduler design](../docs/kernel-k2-7-preemptive-scheduler.md) and
[the K2.8 FPU context design](../docs/kernel-k2-8-fpu-simd-context.md).

## Local checks

```powershell
rustup target add x86_64-unknown-none
cargo test --manifest-path kernel/Cargo.toml
cargo check --manifest-path kernel/Cargo.toml --target x86_64-unknown-none
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld
```

The reproducible QEMU/OVMF build is defined in `.github/workflows/kernel.yml`.
