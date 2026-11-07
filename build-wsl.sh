#!/usr/bin/env bash
set -euo pipefail

PRJ=/mnt/c/Users/eemntirey/Desktop/OSREAL-t8b
WORK=/home/eemntirey/systemes-build-t8b
EFI=$WORK/gnuefi/usr
OUT=$WORK/uefi${SUFFIX:-}
export CARGO_TARGET_DIR=$WORK/target
FEATURES=${FEATURES-qemu-exception-test,qemu-heap-test,qemu-acpi-madt-test,qemu-apic-clock-test,qemu-cooperative-scheduler-test,qemu-preemptive-scheduler-test,qemu-fpu-context-test,qemu-task-sleep-test,qemu-address-space-test,qemu-ring3-test,qemu-syscall-test}
FEATURE_ARGS=()
if [ -n "$FEATURES" ]; then FEATURE_ARGS=(--features "$FEATURES"); fi

mkdir -p "$WORK" "$OUT"
cd "$WORK"

if [ ! -f gnu-efi_4.0.0-1_amd64.deb ]; then
  apt-get download gnu-efi
fi
rm -rf "$WORK/gnuefi"
mkdir -p "$WORK/gnuefi"
dpkg-deb -x gnu-efi_4.0.0-1_amd64.deb "$WORK/gnuefi"

cd "$PRJ"
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none \
  "${FEATURE_ARGS[@]}" \
  --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld

KERNEL_ELF="$CARGO_TARGET_DIR/x86_64-unknown-none/release/systemes-kernel"
objcopy -O binary "$KERNEL_ELF" "$OUT/KERNEL.BIN"
test -s "$OUT/KERNEL.BIN"
kernel_end=$(nm -n "$KERNEL_ELF" | awk '$3 == "__kernel_image_end" { print "0x" $1 }')
test -n "$kernel_end"
kernel_image_bytes=$((kernel_end - 0x00200000))
test "$kernel_image_bytes" -ge "$(stat -c %s "$OUT/KERNEL.BIN")"

gcc -g -O0 -DKERNEL_IMAGE_BYTES=$kernel_image_bytes \
  -I"$EFI/include/efi" -I"$EFI/include/efi/x86_64" \
  -DEFI_FUNCTION_WRAPPER -fpic -ffreestanding -fno-stack-protector \
  -fno-stack-check -fshort-wchar -mno-red-zone -maccumulate-outgoing-args \
  -c boot/uefi-loader/main.c -o "$OUT/loader.o"
ld -nostdlib -znocombreloc -T "$EFI/lib/elf_x86_64_efi.lds" -shared -Bsymbolic \
  "$EFI/lib/crt0-efi-x86_64.o" "$OUT/loader.o" -L"$EFI/lib" -lefi -lgnuefi \
  -o "$OUT/loader.so"
objcopy -j .text -j .sdata -j .data -j .rodata -j .dynamic -j .dynsym -j .rel \
  -j .rela -j .reloc -O efi-app-x86_64 "$OUT/loader.so" "$OUT/BOOTX64.EFI"
test -s "$OUT/BOOTX64.EFI"

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
xorriso -as mkisofs -R -J -V SYSTEMES -o "$OUT/systemes-k1.iso" -e esp.img -no-emul-boot "$OUT/iso-root" >/dev/null
test -s "$OUT/systemes-k1.iso"

echo "BUILD_OK"
echo "KERNEL_IMAGE_BYTES=$kernel_image_bytes"
echo "KERNEL_FILE_BYTES=$(stat -c %s "$OUT/KERNEL.BIN")"
echo "ISO_BYTES=$(stat -c %s "$OUT/systemes-k1.iso")"
