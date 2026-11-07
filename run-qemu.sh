#!/usr/bin/env bash
set -uo pipefail

OUT=${1:-/home/eemntirey/systemes-build/uefi}
firmware=$(find /usr/share/OVMF -maxdepth 1 -type f -name 'OVMF_CODE*.fd' | sort | head -n 1)
variables=$(find /usr/share/OVMF -maxdepth 1 -type f -name 'OVMF_VARS*.fd' | sort | head -n 1)
echo "FIRMWARE=$firmware"
echo "VARIABLES=$variables"
cp "$variables" "$OUT/OVMF_VARS.fd"
rm -f "$OUT/serial.log" "$OUT/debug.log" "$OUT/qemu.log"

timeout 25s qemu-system-x86_64 \
  -cpu max -machine q35 -m 512M \
  -drive if=pflash,format=raw,unit=0,readonly=on,file="$firmware" \
  -drive if=pflash,format=raw,unit=1,file="$OUT/OVMF_VARS.fd" \
  -drive file="$OUT/systemes-k1.iso",media=cdrom,if=ide,readonly=on \
  -boot order=d \
  -serial file:"$OUT/serial.log" \
  -debugcon file:"$OUT/debug.log" \
  -global isa-debugcon.iobase=0xe9 \
  -d int,cpu_reset -D "$OUT/qemu.log" \
  -display none -no-reboot -no-shutdown
status=$?
echo "QEMU_STATUS=$status"
echo "=== DEBUG LOG ==="
cat "$OUT/debug.log" || true
echo "=== SERIAL LOG ==="
cat "$OUT/serial.log" || true
echo "=== MARKERS ==="
for marker in \
  SYSTEMES_UEFI_ENTRY_REACHED \
  SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK \
  'SYSTEMES: kernel entry reached' \
  'SYSTEMES: physical frame allocator initialized' \
  'SYSTEMES: W^X 4 KiB page permissions verified' \
  'SYSTEMES: 1 MiB dynamic kernel heap initialized' \
  'SYSTEMES: ACPI MADT parsed' \
  'SYSTEMES: Local APIC timer calibrated' \
  'SYSTEMES_TEST: Local APIC timer delivered three ticks' \
  'SYSTEMES_TEST: cooperative scheduler switched three kernel tasks' \
  'SYSTEMES_TEST: Local APIC preempted non-yielding tasks' \
  'SYSTEMES_TEST: x87 state survived' \
  'SYSTEMES_TEST: XMM15 state survived' \
  'SYSTEMES: breakpoint handled, execution resumed' \
  'SYSTEMES_TEST: breakpoint returned to caller through iretq' \
  'SYSTEMES: process address space created' \
  'SYSTEMES_TEST: user page mapped with U/S and survived CR3 switch' \
  'SYSTEMES_TEST: kernel identity mapping shared by the process address space' \
  'SYSTEMES: supervisor access protection smep=1 smap=1' \
  'SYSTEMES_TEST: user page access succeeded with SMAP enabled and AC scoped' \
  SYSTEMES_KERNEL_OK; do
  if grep -qF "$marker" "$OUT/debug.log"; then echo "OK   $marker"; else echo "MISS $marker"; fi
done
