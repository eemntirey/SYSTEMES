# SYSTEMES UEFI loader — K1

The GNU-EFI x86_64 application reads `EFI/BOOT/KERNEL.BIN` from the EFI System
Partition, reserves the kernel's linked physical address (`0x00200000`), and
copies the flat kernel image there. It discovers the GOP framebuffer and ACPI RSDP,
captures the UEFI memory map, and calls `ExitBootServices` with bounded retry if
the firmware reports a stale map key.

The loader passes the version-2 `BootInfo` structure to the kernel using the x86_64
System V calling convention. The kernel validates the handoff and emits its boot
milestones through QEMU's debug port `0xE9`. The framebuffer marker is written only
for GOP RGB/BGR pixel formats.

## K1 validation

K1 has passed the GitHub Actions QEMU/OVMF boot test. The test requires all of these
markers in the debug trace:

- `SYSTEMES_UEFI_ENTRY_REACHED`
- `SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK`
- `SYSTEMES: kernel entry reached`
- `SYSTEMES_KERNEL_OK`

See [K1 boot evidence](../docs/K1_BOOT_PROOF.md).

## Explicit limitations

This first implementation is intentionally limited to QEMU/OVMF x86_64:

- Fixed kernel load address at 2 MiB; no relocation or ELF program-header loader
- Firmware page tables remain active; identity mapping is assumed
- No kernel-owned GDT/IDT, interrupt handlers, allocator, scheduler, drivers, or user mode
- ACPI RSDP is located and passed through; ACPI tables are not parsed yet
- GOP is used only for a basic framebuffer marker, not a graphics stack
- No physical-PC compatibility claim has been made

These constraints must be addressed before broad hardware support.

## Build prerequisites

Ubuntu packages: `gnu-efi binutils dosfstools mtools xorriso qemu-system-x86 ovmf`.
Rust target: `x86_64-unknown-none`.

See `.github/workflows/kernel.yml` for the reproducible ISO build and QEMU smoke test.
