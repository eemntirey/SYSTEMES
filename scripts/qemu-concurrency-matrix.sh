#!/usr/bin/env bash
# Functional QEMU matrix for SYSTEMES SMP bring-up and scheduler migration.
# Run from any directory with: bash scripts/qemu-concurrency-matrix.sh
# Optional: CPUS="2 4" TIMEOUT=180 bash scripts/qemu-concurrency-matrix.sh
set -Eeuo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BUILD="$ROOT/build"
LOG="$BUILD/uefi/debug.log"
CPUS=${CPUS:-"1 2 4"}
TIMEOUT=${TIMEOUT:-120}
RESULTS=()
FAILED=0

die() { echo "ERROR: $*" >&2; exit 2; }
[[ "$TIMEOUT" =~ ^[1-9][0-9]*$ ]] || die "TIMEOUT must be a positive integer"

read -r -a CPU_LIST <<< "$CPUS"
(("${#CPU_LIST[@]}" > 0)) || die "CPUS must contain at least one value"
declare -A SEEN=()
for n in "${CPU_LIST[@]}"; do
  [[ "$n" =~ ^(1|2|4)$ ]] || die "unsupported CPU count '$n' (allowed: 1 2 4)"
  [[ -z "${SEEN[$n]:-}" ]] || die "duplicate CPU count '$n'"
  SEEN[$n]=1
done

has() { grep -qF -- "$1" "$LOG"; }
count_matches() {
  local pattern=$1
  grep -Ec "$pattern" "$LOG" || true
}
record() {
  local name=$1 n=$2 verdict=$3 notes=$4
  RESULTS+=("$(printf '%-28s smp=%-2s %-4s %s' "$name" "$n" "$verdict" "$notes")")
  [[ "$verdict" == PASS ]] || FAILED=1
}

run_case() {
  local name=$1 n=$2 mode=$3
  shift 3
  local output="$BUILD/matrix-$name-$n.out"
  local snapshot="$BUILD/matrix-$name-$n.debug.log"
  local verdict=PASS notes="" rc=0

  echo "=== case=$name smp=$n ==="
  mkdir -p "$BUILD"
  # A failed build must never leave a previous run's debug log eligible for PASS.
  rm -f "$LOG" "$snapshot"
  set +e
  bash "$ROOT/scripts/qemu-boot.sh" --smp "$n" --timeout "$TIMEOUT" "$@" >"$output" 2>&1
  rc=$?
  set -e

  if [[ "$rc" -ne 0 ]]; then
    verdict=FAIL
    notes+=" qemu-boot-exit=$rc;"
  fi
  if [[ ! -s "$LOG" ]]; then
    verdict=FAIL
    notes+=" debug.log absent or empty;"
  else
    cp "$LOG" "$snapshot"
    if has 'SYSTEMES_KERNEL_FAIL:'; then
      verdict=FAIL
      notes+=" kernel-failure-marker;"
    fi
    if ! has 'SYSTEMES_KERNEL_OK'; then
      verdict=FAIL
      notes+=" missing-SYSTEMES_KERNEL_OK;"
    fi
  fi

  if [[ -s "$LOG" ]]; then
    case "$mode" in
    smoke)
      # The single-CPU baseline is a normal production boot, not an SMP feature:
      # both SMP features deliberately require at least one application processor.
      ;;
    smp)
      local expected_aps=$((n - 1))
      local ap_count
      ap_count=$(count_matches 'SYSTEMES_TEST: AP [0-9]+ started in kernel long mode')
      [[ "$ap_count" -eq "$expected_aps" ]] || {
        verdict=FAIL; notes+=" AP-count=$ap_count expected=$expected_aps;"
      }
      has "SYSTEMES_TEST: application processors parked after validation: count=$expected_aps" || {
        verdict=FAIL; notes+=" missing-AP-park-count;"
      }
      has "SYSTEMES_TEST: cross-core SpinLock counter verified across $n CPUs" || {
        verdict=FAIL; notes+=" missing-SpinLock-counter-proof;"
      }
      has 'SYSTEMES_TEST: cross-core relaxed counter verified' || {
        verdict=FAIL; notes+=" missing-relaxed-counter-proof;"
      }
      has 'SYSTEMES_TEST: per-AP private counters verified' || {
        verdict=FAIL; notes+=" missing-private-counter-proof;"
      }
      has 'SYSTEMES_TEST: SMP bring-up validation complete' || {
        verdict=FAIL; notes+=" missing-SMP-complete-marker;"
      }
      ;;
    ring3-fault-pf|ring3-fault-gp)
      local expected_aps=$((n - 1))
      local ap_count
      ap_count=$(count_matches 'SYSTEMES_TEST: AP [0-9]+ started in kernel long mode')
      [[ "$ap_count" -eq "$expected_aps" ]] || { verdict=FAIL; notes+=" AP-count=$ap_count expected=$expected_aps;"; }
      has 'SYSTEMES_TEST: Ring 3 fault detected' || { verdict=FAIL; notes+=" missing-fault-detected;"; }
      if [[ "$name" == qemu-ring3-fault-pf ]]; then
        grep -Eq 'SYSTEMES_TEST: Ring 3 fault detected vector=0x0*E' "$LOG" || { verdict=FAIL; notes+=" missing-#PF-vector;"; }
        grep -Eq 'cr2=0x[0-9A-Fa-f]+' "$LOG" || { verdict=FAIL; notes+=" missing-#PF-CR2;"; }
      else
        grep -Eq 'SYSTEMES_TEST: Ring 3 fault detected vector=0x0*D' "$LOG" || { verdict=FAIL; notes+=" missing-#GP-vector;"; }
      fi
      has 'SYSTEMES_TEST: Ring 3 fault returned to kernel; repeated=10' || { verdict=FAIL; notes+=" missing-fault-return-repeated;"; }
      has 'SYSTEMES_TEST: Ring 3 fault cleanup complete' || { verdict=FAIL; notes+=" missing-fault-cleanup;"; }
      has 'SYSTEMES_TEST: non-faulting CPU participants completed count=' || { verdict=FAIL; notes+=" missing-peer-completion;"; }
      has 'SYSTEMES_TEST: frame and stack allocation counters unchanged' || { verdict=FAIL; notes+=" frame-or-stack-leak;"; }
      ;;
    syscall)
      has 'SYSTEMES_TEST: invalid syscall numbers and user pointers returned errors' || { verdict=FAIL; notes+=" missing-EFAULT-syscall-proof;"; }
      has 'SYSTEMES_TEST: user process exited and kernel resumed' || { verdict=FAIL; notes+=" user-context-was-not-resumed;"; }
      ;;
    ring3)
      local expected_aps=$((n - 1))
      local ap_count
      ap_count=$(count_matches 'SYSTEMES_TEST: AP [0-9]+ started in kernel long mode')
      [[ "$ap_count" -eq "$expected_aps" ]] || {
        verdict=FAIL; notes+=" AP-count=$ap_count expected=$expected_aps;"
      }
      has "SYSTEMES_TEST: Ring 3 AP rendezvous completed: MADT APs=$expected_aps, Ring 3 APs=$expected_aps" || {
        verdict=FAIL; notes+=" missing-MADT-to-Ring3-rendezvous-proof;"
      }
      if grep -Eq "NoLowMemoryPage|no-low-memory-page" "$LOG"; then
        verdict=FAIL; notes+=" NoLowMemoryPage-marker;"
      fi
      has "SYSTEMES_TEST: simultaneous Ring 3 SYSCALL and per-CPU exit contexts verified across $n CPUs" || {
        verdict=FAIL; notes+=" missing-simultaneous-ring3-syscall-proof;"
      }
      has 'SYSTEMES_TEST: Ring 3-origin APIC timer interrupts verified on IF-enabled CPUs' || {
        verdict=FAIL; notes+=" missing-ring3-timer-interrupt-proof;"
      }
      ;;
    scheduler)
      local expected_aps=$((n - 1))
      local ap_count migrations resumed
      ap_count=$(count_matches 'SYSTEMES_TEST: AP [0-9]+ started in kernel long mode')
      [[ "$ap_count" -eq "$expected_aps" ]] || {
        verdict=FAIL; notes+=" AP-count=$ap_count expected=$expected_aps;"
      }
      has 'SYSTEMES: per-CPU schedulers initialized; work-stealing enabled' || {
        verdict=FAIL; notes+=" missing-per-CPU-scheduler-marker;"
      }
      migrations=$(sed -nE 's/.*SYSTEMES_TEST: cross-CPU work-stealing migration confirmed count=([0-9]+).*/\1/p' "$LOG" | tail -n1)
      resumed=$(sed -nE 's/.*SYSTEMES_TEST: migrated task resumed after repeated migrations count=([0-9]+).*/\1/p' "$LOG" | tail -n1)
      [[ "${migrations:-0}" =~ ^[0-9]+$ ]] && (( migrations >= 2 )) || {
        verdict=FAIL; notes+=" migrations=${migrations:-missing} expected>=2;"
      }
      [[ "${resumed:-0}" =~ ^[0-9]+$ ]] && (( resumed >= 2 )) || {
        verdict=FAIL; notes+=" resumed=${resumed:-missing} expected>=2;"
      }
      ;;
    *) die "internal error: unknown mode '$mode'" ;;
    esac
  fi

  record "$name" "$n" "$verdict" "$notes"
  echo "case=$name smp=$n verdict=$verdict$notes"
  if [[ "$verdict" != PASS ]]; then
    echo "--- qemu-boot output: $output" >&2
    tail -n 80 "$output" >&2 || true
    [[ -f "$LOG" ]] && tail -n 80 "$LOG" >&2 || true
  fi
}

for n in "${CPU_LIST[@]}"; do
  if [[ "$n" == 1 ]]; then
    run_case single-core-smoke 1 smoke
    run_case qemu-syscall-test 1 syscall --features qemu-syscall-test --debug-exit
  fi
  run_case qemu-ring3-smp-test "$n" ring3 --features qemu-ring3-smp-test --debug-exit
  run_case qemu-ring3-fault-pf "$n" ring3-fault-pf --features qemu-ring3-fault-pf-test --debug-exit
  run_case qemu-ring3-fault-gp "$n" ring3-fault-gp --features qemu-ring3-fault-gp-test --debug-exit
  if [[ "$n" != 1 ]]; then
    run_case qemu-smp-test "$n" smp --features qemu-smp-test
    run_case qemu-smp-scheduler-test "$n" scheduler --features qemu-smp-scheduler-test --debug-exit
  fi
done

echo
echo "================ SMP CONCURRENCY MATRIX ================"
printf '%s\n' "${RESULTS[@]}"
if [[ "$FAILED" -eq 0 ]]; then
  echo "MATRIX_OK"
else
  echo "MATRIX_FAILED" >&2
  exit 1
fi
