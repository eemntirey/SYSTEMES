#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$PWD"
BUILD="$ROOT/build/uefi"
EFI="$ROOT/tools/root"
rm -rf "$BUILD"
mkdir -p "$BUILD"

cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none \
  --features qemu-fpu-context-test --bin systemes-kernel -- \
  -C relocation-model=static -C link-arg=-Tlinker.ld

ELF="kernel/target/x86_64-unknown-none/release/systemes-kernel"
objcopy -O binary "$ELF" "$BUILD/KERNEL.BIN"
test -s "$BUILD/KERNEL.BIN"
kernel_end=$(nm -n "$ELF" | awk '$3 == "__kernel_image_end" { print "0x" $1 }')
test -n "$kernel_end"
kernel_image_bytes=$((kernel_end - 0x00200000))
test "$kernel_image_bytes" -ge "$(stat -c %s "$BUILD/KERNEL.BIN")"

gcc -g -O0 -DKERNEL_IMAGE_BYTES=$kernel_image_bytes \
  -I"$EFI/usr/include" -I"$EFI/usr/include/efi" -I"$EFI/usr/include/efi/x86_64" \
  -DEFI_FUNCTION_WRAPPER -fpic -ffreestanding -fno-stack-protector -fno-stack-check \
  -fshort-wchar -mno-red-zone -maccumulate-outgoing-args \
  -c boot/uefi-loader/main.c -o "$BUILD/loader.o"

ld -nostdlib -znocombreloc -T "$EFI/usr/lib/elf_x86_64_efi.lds" -shared -Bsymbolic \
  "$EFI/usr/lib/crt0-efi-x86_64.o" "$BUILD/loader.o" \
  -L"$EFI/usr/lib" -lefi -lgnuefi -o "$BUILD/loader.so"

objcopy -j .text -j .sdata -j .data -j .rodata -j .dynamic -j .dynsym -j .rel \
  -j .rela -j .reloc -O efi-app-x86_64 "$BUILD/loader.so" "$BUILD/BOOTX64.EFI"
test -s "$BUILD/BOOTX64.EFI"

truncate -s 16M "$BUILD/esp.img"
mkfs.vfat "$BUILD/esp.img" >/dev/null
mmd -i "$BUILD/esp.img" ::/EFI ::/EFI/BOOT
mcopy -i "$BUILD/esp.img" "$BUILD/BOOTX64.EFI" ::/EFI/BOOT/BOOTX64.EFI
mcopy -i "$BUILD/esp.img" "$BUILD/KERNEL.BIN" ::/EFI/BOOT/KERNEL.BIN
mkdir -p "$BUILD/iso-root/EFI/BOOT"
cp "$BUILD/BOOTX64.EFI" "$BUILD/iso-root/EFI/BOOT/BOOTX64.EFI"
cp "$BUILD/KERNEL.BIN" "$BUILD/iso-root/EFI/BOOT/KERNEL.BIN"
cp "$BUILD/esp.img" "$BUILD/iso-root/esp.img"
xorriso -as mkisofs -R -J -V SYSTEMES-FPU -o "$BUILD/systemes-fpu.iso" \
  -e esp.img -no-emul-boot "$BUILD/iso-root" >/dev/null
test -s "$BUILD/systemes-fpu.iso"

cp /usr/share/OVMF/OVMF_VARS_4M.fd "$BUILD/OVMF_VARS.fd"
set +e
timeout 30s qemu-system-x86_64 -machine q35 -m 512M \
  -drive if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
  -drive if=pflash,format=raw,unit=1,file="$BUILD/OVMF_VARS.fd" \
  -drive file="$BUILD/systemes-fpu.iso",media=cdrom,if=ide,readonly=on \
  -boot order=d -serial file:"$BUILD/serial.log" -debugcon file:"$BUILD/debug.log" \
  -global isa-debugcon.iobase=0xe9 -d int,cpu_reset -D "$BUILD/qemu.log" \
  -display none -no-reboot -no-shutdown
status=$?
set -e
echo "qemu status=$status"

echo "--- boot markers ---"
grep -E "SYSTEMES|SYSTEMES_TEST|SYSTEMES_KERNEL_FAIL" "$BUILD/debug.log" | tail -20

grep -q 'SYSTEMES_KERNEL_OK' "$BUILD/debug.log"
grep -q 'SYSTEMES_TEST: x87/SSE context preserved across preemptive task switches' "$BUILD/debug.log"
grep -q 'SYSTEMES_TEST: boot task x87/SSE state restored after worker preemption' "$BUILD/debug.log"
grep -q 'SYSTEMES_TEST: three FPU tasks retained distinct control words and XMM registers' "$BUILD/debug.log"
if grep -q 'SYSTEMES_KERNEL_FAIL:' "$BUILD/debug.log"; then
  echo "FAIL: kernel failure marker found"
  exit 1
fi
echo "FPU_QEMU_PASS"
