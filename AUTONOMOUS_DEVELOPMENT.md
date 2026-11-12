# Autonomous Development Log

## Project
SYSTEMES native OS (https://github.com/eemntirey/SYSTEMES)

## Current verified state — 2026-11-12

- Audited branch: `main`.
- Audited HEAD: `a5c89e5d57bca9f874952d96ab801f98df1c27f1`.
- Latest commit title claims `SYS_SPAWN`, `SYS_FORK`, `SYS_GETPID`,
  `SYS_WAITPID`, `SYS_READKEY` and a PS/2 driver.
- **Those claims are not true of the executable kernel on this HEAD.**
- `kernel/src/syscall.rs` exposes syscall numbers 0–8 only. The five
  advertised names are absent from the constants and dispatcher.
- `kernel/src/process.rs` and `kernel/src/keyboard.rs` exist but neither
  module is declared in `kernel/src/lib.rs`; Cargo does not compile them as
  part of the kernel crate. `fork_current()` is explicitly a stub; the
  keyboard methods are no-op stubs (`init() -> Ok("ok")`, `poll() -> 0`).
- There is no demonstrated PS/2 controller initialization, IRQ1 handler,
  keyboard event queue, process syscall integration, or wait/reap path.
- The existing Ring 3 shell, file syscalls, UEFI boot and SMP work remain
  distinct, already-integrated capabilities; do not conflate them with the
  new commit's unintegrated files.

### Validation evidence on the exact HEAD

GitHub Actions run #423
(https://github.com/eemntirey/SYSTEMES/actions/runs/43224174568) completed
successfully on `a5c89e5d57bca9f874952d96ab801f98df1c27f1`:

- 13/13 check-runs succeeded.
- Host contract tests: CI step `cargo test --all-features --manifest-path
  kernel/Cargo.toml` succeeded.
- Target check: CI step `cargo check --all-features --manifest-path
  kernel/Cargo.toml --target x86_64-unknown-none` succeeded.
- UEFI/OVMF boot, production shell (with and without SYSFS), hardware-fault
  QEMU jobs, SMP bring-up and QEMU 1/2/4 CPU matrix succeeded.
- These results do **not** compile or validate `process.rs` or
  `keyboard.rs`, because those modules are not in the crate module graph.
  No process or keyboard-specific QEMU marker is asserted by the workflow.

### Current phase and next step

**Current phase: audit/correction of the advertised process and keyboard work.**

The next implementation work must be split into targeted PRs:
1. Process/syscall integration: define stable syscall numbers and ABI first;
   export and compile process code; implement and test spawn, PID, fork
   semantics, wait/reap, per-process resources and safe address-space cleanup.
2. PS/2 keyboard: implement a real bounded 8042/PS/2 driver, scan-code set 2
   decoder, IRQ routing/handler and event queue; then add the `SYS_READKEY`
   contract and Ring 3/QEMU input proof.

Do not call either feature complete until host tests, target compilation and
QEMU/OVMF tests cover the actual behavior. Physical hardware remains a
separate, unproven level. The detailed matrix and acceptance tests are in
[`docs/STATE.md`](docs/STATE.md); the supported syscall contract is in
[`docs/SYSCALL_ABI.md`](docs/SYSCALL_ABI.md).

## Historical development log (archived entries below)

The entries below are retained as historical records. Their "next" statements
and branch/HEAD references are not the current repository state; use the
verified state above and `docs/STATE.md` instead.

## Local Environment
- `gh` CLI: NOT installed
- GitHub PAT: 40 chars, retrievable via `git credential fill` (Windows Credential Manager)
- Helper script: `C:\Users\eemntirey\AppData\Local\Temp\opencode\gh_api.ps1`
- `cargo` 1.98.1, `rust-toolchain` 1.98.1
- `python` 3.11.9 works, `python3` broken
- QEMU: `C:\Program Files\qemu\qemu-system-x86_64.exe`
- NO GNU binutils/gcc/clang locally (CI handles release builds)
- Worktree: `.p1-6-smp-worktree` on branch `p1-6-smp-ap-bringup` (merged, can be removed)

### Changes
| File | Description |
|------|-------------|
| `kernel/smp.rs` | New module: trampoline payload (16/32-bit), INIT/SIPI bring-up, per-AP stacks, cross-core validation (250 rounds private/relaxed/SpinLock counters) |
| `kernel/apic.rs` | `current_apic_id()`, `send_init_and_startup_ipi(apic_id, vector_page)` with bounded IPI delivery waits + PIT delays |
| `kernel/memory.rs` | `allocate_frame_below(limit)` with tests (head-advance, top-cut, page-0 exclusion) |
| `kernel/main.rs` | `qemu-smp-test` feature block with validation markers |
| `kernel/Cargo.toml` | `qemu-smp-test` feature |
| `kernel/lib.rs` | Export `smp` module |
| `kernel/interrupts.rs` | Remove `.type` directives (Rust 1.98 assembler compat) |
| `.github/workflows/kernel.yml` | New `smp-bringup` CI job (`-smp 4`, greps all markers) |
| `scripts/qemu-boot.sh` | `--smp` option for local testing |

### Validation Markers (CI greps)
- `SYSTEMES: AP trampoline published under 1 MiB`
- `SYSTEMES_TEST: AP <n> started in kernel long mode`
- `SYSTEMES_TEST: application processors parked after validation: count=<n>`
- `SYSTEMES_TEST: cross-core SpinLock counter verified across <cpus> CPUs`
- `SYSTEMES_TEST: cross-core relaxed counter verified`
- `SYSTEMES_TEST: per-AP private counters verified`
- `SYSTEMES_TEST: SMP bring-up validation complete`

### Testing Evidence
- Host tests: 135 passed (`cargo test --all-features`)
- Release build: clean (`cargo rustc --release --target x86_64-unknown-none --features qemu-smp-test ...`)
- CI job `smp-bringup`: QEMU with `-smp 4 -cpu max -machine q35`, greps all markers above

## Next Actions
1. ~~Wait for CI completion on PR #75 (head `4ecb21ce`)~~ ✓
2. ~~Verify all mandatory check-runs green (excluding "Kilo Code Review")~~ ✓
3. ~~Merge PR #75~~ ✓ **MERGED as `b3cbbe7e`**
4. Fetch new `main`, rebase/merge with PR #73 if needed
5. **START P1-6b: per-CPU GDT/IDT/TSS + per-CPU APIC timers + multi-core scheduler**

## Repo Rules Compliance
- No `git reset --hard` to erase work
- No force-push
- No deleting other agents' branches
- No direct unvalidated merges to main
- PR policy: dedicated branch → push → PR to main → verify CI on exact head SHA → merge only if all mandatory checks green
- "Ne jamais utiliser Kilo Code Review comme condition de fusion"
- Maintain `AUTONOMOUS_DEVELOPMENT.md` at repo root (created here, will be committed after merge)
- Avoid conflicts with other agents' work (disjoint files: PR #73 touches syscall.rs, vfs.rs, simple_fs.rs, shell.S, shell_elf.rs, kernel.yml, make-sysfs.py; this PR touches smp.rs, apic.rs, memory.rs, main.rs, lib.rs, Cargo.toml, interrupts.rs, kernel.yml, qemu-boot.sh)

## Resume State (if session limits)
- Worktree: `C:\Users\eemntirey\Desktop\OSREAL\.p1-6-smp-worktree` (merged, can be removed)
- Branch: `p1-6-smp-ap-bringup` (merged as `b3cbbe7e`, can be deleted)
- Next: **start P1-6b worktree for per-CPU GDT/IDT/TSS + per-CPU APIC timers + multi-core scheduler**

## 2026-11-09 — trampoline debug session (Codebuff)

The first pushed head booted `SYSTEMES_KERNEL_OK` and then stopped: no AP ever
reached kernel code. QEMU `-d int` traces plus a temporary payload hexdump
identified five independent defects, all fixed in `49f76e5c`:

1. The SIPI page was allocated inside `bring_up_application_processors`, i.e.
   after `build_identity_map` had already consumed the first MiB for its page
   tables. Copying the payload there corrupted those tables (reserved-bit page
   faults). The page is now reserved by `smp::reserve_trampoline_page` right
   after the frame allocator is built, before paging exists.
2. The 32-bit stage set `EFER.LME` but not `EFER.NXE`. Every data leaf of the
   kernel identity map carries NX, so the AP tripped reserved-bit page faults
   on its first MMIO read.
3. 32-bit data accesses add the segment base, and DS still carried the
   trampoline page base: `[lapic_base + 0x20]` became `0xfee08020`. The stage
   now loads flat selectors (`0x18`) before MMIO and before the kernel far jump.
4. The GDTR base was patched with the payload page start instead of the GDT
   symbol inside the page, so `ljmp $0x08` raised `#GP(8)`.
5. The bootstrap CPU waited for exactly `SLOT_STATE_STARTED`; a fast AP had
   already stored `PARKED`, so the wait timed out. It now accepts any state at
   or past `STARTED`.

Evidence on `49f76e5c` (WSL Ubuntu, QEMU/OVMF, `-smp 4 -cpu max -machine q35`),
pushed as `f68bddba` together with this log:

```
SYSTEMES_KERNEL_OK
SYSTEMES_TEST: AP 1 started in kernel long mode
SYSTEMES_TEST: AP 2 started in kernel long mode
SYSTEMES_TEST: AP 3 started in kernel long mode
SYSTEMES_TEST: application processors parked after validation: count=3
SYSTEMES_TEST: cross-core SpinLock counter verified across 4 CPUs
SYSTEMES_TEST: cross-core relaxed counter verified
SYSTEMES_TEST: per-AP private counters verified
SYSTEMES_TEST: SMP bring-up validation complete
```

Host suite on the same tree: 135/135 (`cargo test --all-features`). Physical
hardware remains unvalidated.

Remaining known limits for P1-6a: APs park with interrupts disabled, the GDT,
IDT and TSS are still single static tables, and the scheduler remains
bootstrap-only. P1-6b owns per-CPU descriptor tables, per-CPU APIC timers and
the multi-core scheduler.

## 2026-11-09 — P1-6a Merged, Transition to P1-6b

PR #75 merged successfully as `b3cbbe7e`. All mandatory CI checks passed:
- Host tests: 135/135 passed
- Release build: clean
- `smp-bringup` CI job: QEMU with `-smp 2 -cpu max -machine q35` (180s timeout), all validation markers verified:
  - `SYSTEMES: AP trampoline published under 1 MiB`
  - `SYSTEMES_TEST: AP 1 started in kernel long mode`
  - `SYSTEMES_TEST: application processors parked after validation: count=1`
  - `SYSTEMES_TEST: cross-core SpinLock counter verified across 2 CPUs`
  - `SYSTEMES_TEST: cross-core relaxed counter verified`
  - `SYSTEMES_TEST: per-AP private counters verified`
  - `SYSTEMES_TEST: SMP bring-up validation complete`

Known P1-6a limits (to be addressed in P1-6b):
- APs park with interrupts disabled
- GDT, IDT, TSS are single static tables (shared across CPUs)
- Scheduler runs only on bootstrap CPU
- Only bootstrap CPU services interrupts (APIC timer)

Next phase P1-6b will implement:
1. Per-CPU GDT/IDT/TSS (separate descriptor tables per CPU)
2. Per-CPU APIC timer initialization (each CPU runs its own timer)
3. Multi-core scheduler (work-stealing or per-CPU run queues)

PR #73 (P1-5c — SYS_READDIR + `ls`) still open; other agent to merge and start P2-1 (writable VFS).

## 2026-11-09 — P1-6b: per-CPU GDT/IDT/TSS + per-CPU APIC timers + multi-core scheduler (IN PROGRESS)

**Branch**: `p1-6b-per-cpu-gdt-tss` (worktree `.p1-6b-worktree`)
**Base**: `origin/main` at `61889afa` (post P1-6a merge)

### Implementation Complete (local verification)

| File | Changes |
|------|---------|
| `kernel/interrupts.rs` | `CpuContext[64]` array with per-CPU GDT/IDT/TSS + 5 interrupt stacks; `initialize_cpu(cpu_index)`; verification fns take `cpu_index` |
| `kernel/scheduler.rs` | `CpuScheduler[64]` array with per-CPU run queues; `initialize_cpu(cpu_index, entries)`; APs enter `idle_loop()` calling `yield_now()`; work-stealing stub |
| `kernel/apic.rs` | `initialize_local_timer_on_cpu(initial_count)`, `set_timer_initial_count()`, `get_timer_initial_count()` for BSP→AP calibration sharing |
| `kernel/smp.rs` | APs call `initialize_cpu(slot)` and `initialize_local_timer_on_cpu()` in `systemes_ap_main_rust`; `sti` + idle loop |
| `kernel/main.rs` | P1-6b integration block under `qemu-smp-scheduler-test`: BSP initializes multi-core scheduler after APs join |
| `kernel/Cargo.toml` | `qemu-smp-scheduler-test = ["qemu-smp-test"]` feature |
| `.github/workflows/kernel.yml` | `smp-scheduler` CI job (`-smp 2`, 180s timeout) with new verification markers |

### Verification Markers (CI will grep)
- `SYSTEMES: AP trampoline published under 1 MiB`
- `SYSTEMES_TEST: AP <n> started in kernel long mode`
- `SYSTEMES_TEST: application processors joined scheduler: count=<n>`
- `SYSTEMES_TEST: cross-core SpinLock counter verified across <cpus> CPUs`
- `SYSTEMES_TEST: cross-core relaxed counter verified`
- `SYSTEMES_TEST: per-AP private counters verified`
- `SYSTEMES_TEST: SMP bring-up validation complete`
- `SYSTEMES: multi-core scheduler initialized on BSP`

### Local Validation
- `cargo check --all-features --target x86_64-unknown-none` ✓ clean
- `cargo test --all-features` ✓ 135/135 passed
- `cargo rustc --release --target x86_64-unknown-none --features qemu-smp-scheduler-test --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld` ✓ clean

### Next Steps
1. Push branch → create PR to main (at https://github.com/eemntirey/SYSTEMES/pull/new/p1-6b-per-cpu-gdt-tss)
2. Wait for CI `smp-scheduler` job to complete (QEMU `-smp 2`)
3. Verify all mandatory check-runs green on exact head SHA
4. Merge PR #76 (P1-6b)
5. Update `AUTONOMOUS_DEVELOPMENT.md` with merge commit and CI evidence
6. Begin P1-6c: lock-free per-CPU data structures

## P1-6b PR #76 — Recovery and verified limitations (2026-11-09)

PR: https://github.com/eemntirey/SYSTEMES/pull/76  
Branch: `p1-6b-per-cpu-architecture`

### Confirmed regressions and corrections

- UEFI kernel read: the fixed 8 MiB read limit rejected a kernel image whose linked extent was approximately 15.8 MiB. The loader now queries `EFI_FILE_INFO.FileSize`, allocates to the actual file size, loops over short reads, checks EOF, bounds the file against `KERNEL_IMAGE_BYTES`, and emits size/status diagnostics.
- Physical kernel reservation: the 15.8 MiB image could not be reserved at the old 2 MiB base. The kernel linker base and UEFI load address were moved together to 32 MiB; all five CI image-size calculations were updated. QEMU logs on run 354 showed `SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK` and entry into the kernel.
- Ring 3 / SYSCALL ABI: P1-6b had repurposed RSI/RDI as pointers without changing the Rust/System V call ABI, and omitted the user RSP from the IRETQ frame. The proven BSP assembly ABI was restored: user SS/RSP/RFLAGS/CS/RIP are pushed in architectural order, and the BSP SYSCALL stack/user-RSP and Ring 3 saved-RSP symbols are again addressed explicitly. This is a BSP-only compatibility restoration, not per-CPU syscall support.

### P1-6b functionality that is NOT complete

- Per-CPU GDT/IDT/TSS and AP-local timer setup are present, but AP task schedulers are not initialized.
- APs remain in an interruptible idle loop. The BSP scheduler still schedules only tasks in its own per-CPU array.
- The prior `steal_task` function was an uncalled placeholder that only changed a source task state; it did not transfer a runnable context. It and the unused global task-pool declarations were removed rather than presented as working functionality.
- There is no verified cross-CPU task queue, task ownership transfer, wakeup protocol, or double-execution prevention.
- SYSCALL/SYSRET and Ring 3 currently use the restored BSP-only assembly stack. Per-CPU syscall entry, GS-base switching, and AP user-mode transitions remain unimplemented.

### Validation state

- Run 354 (commit `6f6449d`): UEFI file read succeeded; boot then failed at fixed physical reservation.
- Run 355 (commit `3ec3fc7`): 32 MiB reservation, kernel load, ExitBootServices, and kernel entry succeeded. Production boot reached userland but exposed the Ring 3 frame ABI regression.
- Run 356 (commit `13054a6`): production boots with and without SYSFS passed after restoring the BSP Ring 3/SYSCALL ABI. Several hardware-fault jobs still failed and require exact log-based diagnosis.
- Subsequent runs are pending or in progress; their results must be checked against the exact current PR HEAD before any completion claim.

**P1-6b is not complete. Do not merge PR #76.** The acceptance criteria for a functional multi-core scheduler and cross-CPU task migration remain unmet. No QEMU 1/2/4-CPU scheduler migration proof is available yet.
