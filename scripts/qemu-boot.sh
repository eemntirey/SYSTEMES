#!/usr/bin/env bash
# Build the freestanding kernel + GNU-EFI loader and boot them in QEMU/OVMF.
#
# This script mirrors the `uefi-qemu-boot` CI job so that local evidence matches
# the pipeline. It must run on Linux (or WSL): it needs gcc, ld, objcopy, nm,
# mkfs.vfat, mmd, mcopy, xorriso, QEMU and OVMF.
#
# Usage:
#   scripts/qemu-boot.sh                     # production boot, no qemu-* feature
#   scripts/qemu-boot.sh --ci-features       # same feature set as the CI job
#   scripts/qemu-boot.sh --features qemu-virtio-vfs-test
#   scripts/qemu-boot.sh --disk --input 'help\n'
#   scripts/qemu-boot.sh --smp 4 --features qemu-smp-test
#   scripts/qemu-boot.sh --ci-features --ci-markers
#
# Artifacts: build/uefi/{KERNEL.BIN,BOOTX64.EFI,systemes-k1.iso,debug.log,
#            serial.log,qemu.log}
set -uo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUT="$ROOT/build/uefi"
GNUEFI_WORK="$ROOT/build/gnuefi"

CI_FEATURES="qemu-exception-test,qemu-heap-test,qemu-acpi-madt-test,qemu-apic-clock-test,qemu-cooperative-scheduler-test,qemu-preemptive-scheduler-test,qemu-fpu-context-test,qemu-task-sleep-test,qemu-address-space-test,qemu-ring3-test,qemu-syscall-test,qemu-user-shell-test,qemu-virtio-vfs-test,qemu-user-fault-test"

FEATURES=""
WANT_DISK=0
INPUT=""
SMP_CPUS=1
TIMEOUT=45
CHECK_CI_MARKERS=0
EXPECT=()

while [ $# -gt 0 ]; do
  case "$1" in
    --features) FEATURES="$2"; shift 2 ;;
    --ci-features) FEATURES="$CI_FEATURES"; shift ;;
    --disk) WANT_DISK=1; shift ;;
    --smp) SMP_CPUS="$2"; shift 2 ;;
    --input) INPUT="$2"; shift 2 ;;
    --timeout) TIMEOUT="$2"; shift 2 ;;
    --expect) EXPECT+=("$2"); shift 2 ;;
    --ci-markers) CHECK_CI_MARKERS=1; shift ;;
    --keep) shift ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "unknown option: $1" >&2; exit 2 ;;
  esac
done

need() { command -v "$1" >/dev/null || { echo "ERROR: missing tool: $1" >&2; exit 1; }; }
for tool in cargo objcopy nm gcc ld mkfs.vfat mmd mcopy xorriso qemu-system-x86_64 python3; do need "$tool"; done

mkdir -p "$OUT"

echo "== Build kernel (features='${FEATURES}')"
FEATURE_ARGS=()
if [ -n "$FEATURES" ]; then FEATURE_ARGS=(--features "$FEATURES"); fi
cd "$ROOT"
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none \
  "${FEATURE_ARGS[@]}" --bin systemes-kernel -- \
  -C relocation-model=static -C link-arg=-Tlinker.ld || exit 1

KERNEL_ELF="$ROOT/kernel/target/x86_64-unknown-none/release/systemes-kernel"
objcopy -O binary "$KERNEL_ELF" "$OUT/KERNEL.BIN" || exit 1
test -s "$OUT/KERNEL.BIN" || { echo "ERROR: empty KERNEL.BIN" >&2; exit 1; }
kernel_end=$(nm -n "$KERNEL_ELF" | awk '$3 == "__kernel_image_end" { print "0x" $1 }')
test -n "$kernel_end" || { echo "ERROR: __kernel_image_end missing" >&2; exit 1; }
KERNEL_IMAGE_BYTES=$((kernel_end - 0x00200000))
test "$KERNEL_IMAGE_BYTES" -ge "$(stat -c %s "$OUT/KERNEL.BIN")" \
  || { echo "ERROR: kernel image exceeds reservation" >&2; exit 1; }

echo "== Build GNU-EFI loader"
if [ ! -f "$GNUEFI_WORK/usr/include/efi/efi.h" ]; then
  mkdir -p "$GNUEFI_WORK"
  if [ -f "$GNUEFI_WORK/gnu-efi.deb" ]; then
    :
  else
    ( cd "$GNUEFI_WORK" && apt-get download gnu-efi >/dev/null 2>&1 ) || {
      echo "ERROR: cannot download gnu-efi (need gnu-efi headers)" >&2; exit 1; }
    mv "$GNUEFI_WORK"/gnu-efi_*_amd64.deb "$GNUEFI_WORK/gnu-efi.deb"
  fi
  dpkg-deb -x "$GNUEFI_WORK/gnu-efi.deb" "$GNUEFI_WORK" || exit 1
fi
EFI="$GNUEFI_WORK/usr"
test -f "$EFI/include/efi/efi.h" || { echo "ERROR: gnu-efi headers missing in $EFI" >&2; exit 1; }

gcc -g -O0 -DKERNEL_IMAGE_BYTES=$KERNEL_IMAGE_BYTES \
  -I"$EFI/include/efi" -I"$EFI/include/efi/x86_64" \
  -DEFI_FUNCTION_WRAPPER -fpic -ffreestanding -fno-stack-protector \
  -fno-stack-check -fshort-wchar -mno-red-zone -maccumulate-outgoing-args \
  -c "$ROOT/boot/uefi-loader/main.c" -o "$OUT/loader.o" || exit 1
ld -nostdlib -znocombreloc -T "$EFI/lib/elf_x86_64_efi.lds" -shared -Bsymbolic \
  "$EFI/lib/crt0-efi-x86_64.o" "$OUT/loader.o" -L"$EFI/lib" -lefi -lgnuefi \
  -o "$OUT/loader.so" || exit 1
objcopy -j .text -j .sdata -j .data -j .rodata -j .dynamic -j .dynsym -j .rel \
  -j .rela -j .reloc -O efi-app-x86_64 "$OUT/loader.so" "$OUT/BOOTX64.EFI" || exit 1
test -s "$OUT/BOOTX64.EFI" || exit 1

echo "== Build UEFI bootable ISO"
rm -rf "$OUT/esp.img" "$OUT/iso-root"
truncate -s 16M "$OUT/esp.img"
mkfs.vfat "$OUT/esp.img" >/dev/null
mmd -i "$OUT/esp.img" ::/EFI ::/EFI/BOOT
mcopy -i "$OUT/esp.img" "$OUT/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$OUT/esp.img" "$OUT/KERNEL.BIN" ::/EFI/BOOT/KERNEL.BIN
mkdir -p "$OUT/iso-root/EFI/BOOT"
cp "$OUT/BOOTX64.EFI" "$OUT/iso-root/EFI/BOOT/BOOTX64.EFI"
cp "$OUT/KERNEL.BIN" "$OUT/iso-root/EFI/BOOT/KERNEL.BIN"
cp "$OUT/esp.img" "$OUT/iso-root/esp.img"
xorriso -as mkisofs -R -J -V SYSTEMES -o "$OUT/systemes-k1.iso" -e esp.img \
  -no-emul-boot "$OUT/iso-root" >/dev/null || exit 1
test -s "$OUT/systemes-k1.iso" || exit 1

DISK_ARGS=()
if [ "$WANT_DISK" -eq 1 ]; then
  echo "== Build SYSFS virtio-blk test disk"
  python3 - "$OUT/sysfs.img" <<'PY'
import struct, sys
from pathlib import Path
image = bytearray(3 * 512)
image[0:6] = b"SYSFS1"
struct.pack_into("<IIIII", image, 6, 1, 512, 2, 1, 0)
struct.pack_into("<HIII", image, 512, 1, 0, 0, 0)
struct.pack_into("<HIII", image, 576, 0, 5, 0, 1)
image[590:599] = b"hello.txt"
image[1024:1029] = b"hello"
Path(sys.argv[1]).write_bytes(image)
PY
  test "$(stat -c %s "$OUT/sysfs.img")" -eq 1536 || exit 1
  DISK_ARGS=(-drive if=none,id=virtio-disk,file="$OUT/sysfs.img",format=raw
             -device virtio-blk-pci,drive=virtio-disk,disable-modern=on)
fi

echo "== Boot in QEMU/OVMF (timeout ${TIMEOUT}s)"
firmware=$(find /usr/share/OVMF -maxdepth 1 -type f -name 'OVMF_CODE*.fd' | sort | head -n 1)
variables=$(find /usr/share/OVMF -maxdepth 1 -type f -name 'OVMF_VARS*.fd' | sort | head -n 1)
test -n "$firmware" && test -n "$variables" || { echo "ERROR: OVMF firmware not found" >&2; exit 1; }
cp "$variables" "$OUT/OVMF_VARS.fd"
rm -f "$OUT/serial.log" "$OUT/debug.log" "$OUT/qemu.log"

QEMU_CMD=(qemu-system-x86_64 -cpu max -machine q35 -smp "$SMP_CPUS" -m 512M
  -drive if=pflash,format=raw,unit=0,readonly=on,file="$firmware"
  -drive if=pflash,format=raw,unit=1,file="$OUT/OVMF_VARS.fd"
  -drive file="$OUT/systemes-k1.iso",media=cdrom,if=ide,readonly=on
  "${DISK_ARGS[@]}" -boot order=d -monitor none
  -debugcon file:"$OUT/debug.log" -global isa-debugcon.iobase=0xe9
  -d int,cpu_reset -D "$OUT/qemu.log" -display none -no-reboot -no-shutdown)

if [ -n "$INPUT" ]; then
  # printf interprets the escapes in --input; the shell feeds the serial console.
  # Bytes sent before serial::init() are dropped, and `-d int` slows the boot
  # noticeably, hence the conservative default delay (override with INPUT_DELAY).
  set +e
  { sleep "${INPUT_DELAY:-25}"; printf "$INPUT"; sleep 2; } | timeout "${TIMEOUT}s" "${QEMU_CMD[@]}" -serial stdio > "$OUT/serial.log"
else
  set +e
  timeout "${TIMEOUT}s" "${QEMU_CMD[@]}" -serial file:"$OUT/serial.log"
fi
QEMU_STATUS=$?
set -e
case "$QEMU_STATUS" in
  0|124) : ;;
  *) echo "QEMU exited unexpectedly with status $QEMU_STATUS" >&2
     cat "$OUT/serial.log" 2>/dev/null
     exit 1 ;;
esac

echo "== debug.log"
cat "$OUT/debug.log" 2>/dev/null
echo "== serial.log"
cat "$OUT/serial.log" 2>/dev/null

status=0
if grep -q 'SYSTEMES_KERNEL_FAIL:' "$OUT/debug.log"; then
  echo "FAIL kernel reported SYSTEMES_KERNEL_FAIL:" >&2
  status=1
fi
if grep -qF '\r\n' "$OUT/debug.log"; then
  echo "FAIL debug log contains literal \\r\\n escapes" >&2
  status=1
fi
if ! grep -q 'SYSTEMES_KERNEL_OK' "$OUT/debug.log"; then
  echo "FAIL missing marker: SYSTEMES_KERNEL_OK" >&2
  status=1
fi

for marker in "${EXPECT[@]}"; do
  if grep -qF "$marker" "$OUT/debug.log" 2>/dev/null; then
    echo "OK   $marker"
  else
    echo "MISS $marker" >&2
    status=1
  fi
done

if [ "$CHECK_CI_MARKERS" -eq 1 ]; then
  while IFS= read -r marker; do
    [ -z "$marker" ] && continue
    if grep -qF "$marker" "$OUT/debug.log"; then echo "OK   $marker"; else
      echo "MISS $marker" >&2; status=1; fi
  done <<'MARKERS'
SYSTEMES_UEFI_ENTRY_REACHED
SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK
SYSTEMES: kernel entry reached
SYSTEMES: CPU NX capability detected and EFER.NXE enabled
SYSTEMES: kernel code RX, rodata R-NX, data and BSS RW-NX
SYSTEMES: W^X 4 KiB page permissions verified
SYSTEMES: 1 MiB dynamic kernel heap initialized
SYSTEMES_TEST: dynamic heap allocation, alignment and free coalescing verified
SYSTEMES: ACPI MADT parsed; Local APIC and IOAPIC topology discovered
SYSTEMES: Local APIC timer calibrated from PIT channel 2 at 100 Hz
SYSTEMES_TEST: Local APIC timer delivered three ticks; monotonic clock advanced
SYSTEMES_TEST: cooperative scheduler switched three kernel tasks
SYSTEMES_TEST: all cooperative tasks exited and returned to boot task
SYSTEMES_TEST: Local APIC preempted non-yielding tasks
SYSTEMES_TEST: three CPU-bound tasks completed under timer preemption
SYSTEMES_TEST: x87 state survived timer-driven task switches
SYSTEMES_TEST: XMM15 state survived timer-driven task switches
SYSTEMES_TEST: blocked task slept for four monotonic ticks
SYSTEMES_TEST: timer woke the blocked task at its deadline
SYSTEMES_KERNEL_OK
SYSTEMES: GDT initialized
SYSTEMES: IDT initialized for CPU exceptions 0..31
SYSTEMES: TSS TR=0x18 verified; RSP0/IST1/IST2/IST3 and IDT routing verified
SYSTEMES: user GDT descriptors and DPL3 exit gate verified
SYSTEMES: SYSCALL/SYSRET configured with a dedicated kernel stack
SYSTEMES_TEST: Ring 3 interrupt returned through TSS RSP0
SYSTEMES_TEST: Ring 3 code executed with user CS and writable user stack
SYSTEMES_TEST: Ring 3 user stack write verified
SYSTEMES_TEST: SYSCALL/SYSRET fast path executed
SYSTEMES_TEST: Syscall return value verified in user stack
SYSTEMES_TEST: SYS_YIELD returned to user process
SYSTEMES_TEST: invalid syscall numbers and user pointers returned errors
SYSTEMES_TEST: user process exited and kernel resumed
SYSTEMES_TEST: exited process frames reclaimed
SYSTEMES: killing user process: vector=0x000000000000000E
SYSTEMES_TEST: Ring 3 page fault killed only the user process
SYSTEMES_TEST: fault exit status=142 vector=0x000000000000000E
SYSTEMES_TEST: kernel launched a new user process after the fault
SYSTEMES_TEST: faulted process frames reclaimed
SYSTEMES_TEST: user shell address space activated
SYSTEMES_TEST: user SYS_WRITE succeeded
SYSTEMES_TEST: user SYS_READ received UART input
SYSTEMES_TEST: virtio-blk DMA sector read verified
SYSTEMES_TEST: virtio SYSFS mount/open/read/close verified
SYSTEMES_EXCEPTION: vector=0x0000000000000003 error=0x0000000000000000
SYSTEMES: breakpoint handled, execution resumed
SYSTEMES_TEST: breakpoint returned to caller through iretq
SYSTEMES: process address space created
SYSTEMES_TEST: user page mapped with U/S and survived CR3 switch
SYSTEMES_TEST: kernel identity mapping shared by the process address space
SYSTEMES: supervisor access protection smep=1 smap=1
SYSTEMES_TEST: user page access succeeded with SMAP enabled and AC scoped
MARKERS
  if ! grep -qF '> Z' "$OUT/serial.log"; then
    echo "MISS serial marker: '> Z'" >&2; status=1
  fi
fi

if [ "$status" -eq 0 ]; then echo "BOOT_PROOF_OK"; else echo "BOOT_PROOF_FAILED" >&2; fi
exit "$status"
