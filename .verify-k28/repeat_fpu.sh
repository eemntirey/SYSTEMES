#!/usr/bin/env bash
set -uo pipefail
B="$(dirname "$0")/build/uefi"
cd "$(dirname "$0")"
pass=0; fail=0
for i in 1 2 3 4 5 6; do
  cp build/uefi/OVMF_VARS.fd "$B/vars-$i.fd"
  timeout 20s qemu-system-x86_64 -machine q35 -m 512M \
    -drive if=pflash,format=raw,unit=0,readonly=on,file=/usr/share/OVMF/OVMF_CODE_4M.fd \
    -drive if=pflash,format=raw,unit=1,file="$B/vars-$i.fd" \
    -drive file="$B/systemes-fpu.iso",media=cdrom,if=ide,readonly=on \
    -boot order=d -serial file:"$B/serial-$i.log" -debugcon file:"$B/debug-$i.log" \
    -global isa-debugcon.iobase=0xe9 -display none -no-reboot -no-shutdown >/dev/null 2>&1
  if grep -aq "three FPU tasks retained distinct control words" "$B/debug-$i.log"; then pass=$((pass+1)); echo "run $i: PASS"; else fail=$((fail+1)); echo "run $i: FAIL $(grep -a SYSTEMES_KERNEL_FAIL "$B/debug-$i.log" | tail -1)"; fi
done
echo "pass=$pass fail=$fail"
