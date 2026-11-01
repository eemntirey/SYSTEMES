# SYSTEMES UEFI loader — K1

The first loader is a GNU-EFI x86_64 application. It reads the flat kernel image from
`EFI/BOOT/KERNEL.BIN`, reserves its linked physical address (2 MiB), discovers GOP
framebuffer and ACPI RSDP, captures the UEFI memory map, and calls
`ExitBootServices` with bounded retry on a stale map key.

The loader passes the version-2 `BootInfo` structure to the kernel using the x86_64
System V calling convention. The kernel validates the handoff and writes its boot
milestones to COM1. The framebuffer is only touched for GOP RGB/BGR pixel formats.

This is a deliberately narrow QEMU/OVMF target. The 2 MiB fixed load address,
identity-mapped UEFI handoff, and framebuffer assumptions must be revisited before
supporting arbitrary firmware or physical PCs.

## Build prerequisites

Ubuntu packages: `gnu-efi binutils dosfstools mtools xorriso qemu-system-x86 ovmf`.
Rust target: `x86_64-unknown-none`.

See `.github/workflows/kernel.yml` for the reproducible build and QEMU smoke test.
