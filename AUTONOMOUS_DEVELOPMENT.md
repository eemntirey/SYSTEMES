# Autonomous Development Log

## Project
SYSTEMES native OS (https://github.com/eemntirey/SYSTEMES)

## Roadmap Phase
Current: **Phase B (deferred SMP) — P1-6a complete, P1-6b next**
Per `docs/PLAN_PROCHAINE_ETAPE.md`:
- Phase A: Freestanding kernel contract ✓ (merged up to PR #74)
- Phase B: SMP + scheduling + memory
  - P1-6a: SMP AP bring-up (INIT/SIPI) — **THIS PR #75**
  - P1-6b: per-CPU GDT/IDT/TSS + per-CPU APIC timers + multi-core scheduler
  - P1-6c: lock-free per-CPU data structures
- Phase C: VFS + syscalls + userland
  - P2-1: writable VFS (other agent's PR #73)
  - P2-2: extended syscall surface
  - ...

## Remote State Verification
- `origin/main` = `38e30add` (Merge PR #74 from t11-elf-loader) — confirmed matching user's starting point
- PR #73 (P1-5c — SYS_READDIR + `ls`) open on `p1-5c-ls-readdir` (`83a65649`), ALL technical CI ✓, only "Kilo Code Review: action_required"
- Other agent owns PR #73, plans to merge then start P2-1 (writable VFS)
- **This agent chose parallel independent workstream: P1-6a SMP AP bring-up** (files disjoint from PR #73)

## Local Environment
- `gh` CLI: NOT installed
- GitHub PAT: 40 chars, retrievable via `git credential fill` (Windows Credential Manager)
- Helper script: `C:\Users\eemntirey\AppData\Local\Temp\opencode\gh_api.ps1`
- `cargo` 1.98.1, `rust-toolchain` 1.98.1
- `python` 3.11.9 works, `python3` broken
- QEMU: `C:\Program Files\qemu\qemu-system-x86_64.exe`
- NO GNU binutils/gcc/clang locally (CI handles release builds)
- Worktree: `.p1-6-smp-worktree` on branch `p1-6-smp-ap-bringup` (based on origin/main 38e30add)

## PR #75: P1-6 SMP Application Processor Bring-up (P1-6a)
- Branch: `p1-6-smp-ap-bringup` (head `4ecb21ce`)
- Target: `main` (base `38e30add`)
- Status: **CI running** (check-run "SMP application processor bring-up" in_progress)

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
1. Wait for CI completion on PR #75 (head `4ecb21ce`)
2. Verify all mandatory check-runs green (excluding "Kilo Code Review")
3. Merge PR #75
4. Fetch new `main`, rebase/merge with PR #73 if needed
5. Continue to P1-6b: per-CPU GDT/IDT/TSS + per-CPU APIC timers + multi-core scheduler

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
- Worktree: `C:\Users\eemntirey\Desktop\OSREAL\.p1-6-smp-worktree`
- Branch: `p1-6-smp-ap-bringup` (pushed, PR #75 open)
- Head SHA: `f68bddba` (pushed: trampoline fix + evidence docs)
- CI status: re-run required on the new head
- Next: monitor CI, merge when green, continue to P1-6b

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