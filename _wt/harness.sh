#!/usr/bin/env bash
# Local reproduction of the kernel.yml QEMU feature tests.
# Usage: harness.sh <source-root> <feature> [required marker ...]
set -euo pipefail

SRC="$1"
FEATURE="$2"
shift 2
MARKERS=("$@")

GNUE="$HOME/gnuefi/usr"
WORK="${WORK:-$HOME/kwork-$(echo "$FEATURE" | tr -d '-')}"

rm -rf "$WORK"
mkdir -p "$WORK/build/uefi"
cp -a "$SRC/kernel" "$SRC/boot" "$WORK/"

export PATH="$HOME/.cargo/bin:$PATH"
cd "$WORK"

echo "== build kernel feature=$FEATURE =="
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none \
  --features "$FEATURE" --bin systemes-kernel -- \
  -C relocation-model=static -C link-arg=-Tlinker.ld

KERNEL_ELF=kernel/target/x86_64-unknown-none/release/systemes-kernel
objcopy -O binary "$KERNEL_ELF" build/uefi/KERNEL.BIN
test -s build/uefi/KERNEL.BIN
kernel_end=$(nm -n "$KERNEL_ELF" | awk '$3 == "__kernel_image_end" { print "0x" $1 }')
kernel_image_bytes=$((kernel_end - 0x00200000))
test "$kernel_image_bytes" -ge "$(stat -c %s build/uefi/KERNEL.BIN)"
echo "KERNEL_IMAGE_BYTES=$kernel_image_bytes"

echo "== build gnu-efi loader =="
gcc -g -O0 -DKERNEL_IMAGE_BYTES=$kernel_image_bytes \
  -I"$GNUE/include/efi" -I"$GNUE/include/efi/x86_64" -DEFI_FUNCTION_WRAPPER -fpic -ffreestanding \
  -fno-stack-protector -fno-stack-check -fshort-wchar -mno-red-zone -maccumulate-outgoing-args \
  -c boot/uefi-loader/main.c -o build/uefi/loader.o
ld -nostdlib -znocombreloc -T "$GNUE/lib/elf_x86_64_efi.lds" -shared -Bsymbolic \
  "$GNUE/lib/crt0-efi-x86_64.o" build/uefi/loader.o -L"$GNUE/lib" -lefi -lgnuefi \
  -o build/uefi/loader.so
objcopy -j .text -j .sdata -j .data -j .rodata -j .dynamic -j .dynsym -j .rel -j .rela \
  -j .reloc -O efi-app-x86_64 build/uefi/loader.so build/uefi/BOOTX64.EFI
test -s build/uefi/BOOTX64.EFI

echo "== build ISO =="
truncate -s 16M build/uefi/esp.img
mkfs.vfat build/uefi/esp.img >/dev/null
mmd -i build/uefi/esp.img ::/EFI ::/EFI/BOOT
mcopy -i build/uefi/esp.img build/uefi/BOOTX64.EFI ::/EFI/BOOT/BOOTX64.EFI
mcopy -i build/uefi/esp.img build/uefi/KERNEL.BIN ::/EFI/BOOT/KERNEL.BIN
mkdir -p build/uefi/iso-root/EFI/BOOT
cp build/uefi/BOOTX64.EFI build/uefi/KERNEL.BIN build/uefi/iso-root/EFI/BOOT/
cp build/uefi/esp.img build/uefi/iso-root/esp.img
xorriso -as mkisofs -R -J -V SYSTEMES -o build/uefi/systemes.iso -e esp.img \
  -no-emul-boot build/uefi/iso-root >/dev/null

echo "== boot QEMU/OVMF =="
firmware="$(find /usr/share/OVMF -maxdepth 1 -type f -name 'OVMF_CODE*.fd' | sort | head -n 1)"
variables="$(find /usr/share/OVMF -maxdepth 1 -type f -name 'OVMF_VARS*.fd' | sort | head -n 1)"
cp "$variables" build/uefi/OVMF_VARS.fd
set +e
timeout 30s qemu-system-x86_64 -machine q35 -m 512M \
  -drive if=pflash,format=raw,unit=0,readonly=on,file="$firmware" \
  -drive if=pflash,format=raw,unit=1,file=build/uefi/OVMF_VARS.fd \
  -drive file=build/uefi/systemes.iso,media=cdrom,if=ide,readonly=on -boot order=d \
  -serial file:build/uefi/serial.log -debugcon file:build/uefi/debug.log \
  -global isa-debugcon.iobase=0xe9 -display none -no-reboot -no-shutdown
status=$?
set -e
echo "QEMU exit status: $status (124 = timeout as expected)"

echo "== debug log tail =="
tail -n 60 build/uefi/debug.log
if grep -q 'SYSTEMES_KERNEL_FAIL' build/uefi/debug.log; then
  echo "FAILURE MARKER PRESENT"
  exit 1
fi
for marker in "${MARKERS[@]}"; do
  if ! grep -qF "$marker" build/uefi/debug.log; then
    echo "MISSING MARKER: $marker"
    exit 1
  fi
done
echo "ALL MARKERS PRESENT"
