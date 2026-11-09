# SYSTEMES — Autonomous Development Log

## Repository State
- **Remote**: https://github.com/eemntirey/SYSTEMES
- **HEAD (main)**: `38e30add` — Merge PR #74 from `t11-elf-loader`
- **Last updated**: 2026-11-09

---

## Completed Phases

### Phase A — Kernel Foundation ✅
| Feature | PR | Status |
|---|---|---|
| x86_64 GDT, IDT, exceptions | #multiple | ✅ Verified in QEMU/CI |
| Physical memory allocator | #49 | ✅ |
| Virtual memory, paging, NX/W^X | #multiple | ✅ SMEP/SMAP verified |
| Dynamic kernel heap (1 MiB) | #49 | ✅ |
| TSS/IST exception stacks | #57 | ✅ |
| ACPI/MADT discovery | #50 | ✅ |
| Local APIC timer, monotonic clock | #51 | ✅ |
| Serial UART console | merged | ✅ |

### Phase B — Multitasking ✅
| Feature | PR | Status |
|---|---|---|
| Cooperative scheduler | #52 | ✅ 3 tasks verified |
| Preemptive scheduler (APIC timer) | #53 | ✅ |
| FPU/SIMD context isolation | #54 | ✅ x87+XMM15 verified |
| Task sleep/wakeup | #57 | ✅ 4-tick sleep verified |

### Phase C — User Mode Execution ✅
| Feature | PR | Status |
|---|---|---|
| Process address spaces | #61 | ✅ |
| SMEP/SMAP + uaccess helpers | #61 | ✅ |
| Ring 3 transition | T9 | ✅ |
| SYSCALL/SYSRET + ABI | T10 | ✅ |
| ELF64 static loader | #68/#74 | ✅ |
| Userland shell (assembly, Ring 3) | #74 | ✅ Running |
| Syscalls: read/write/exit/yield/uptime/open/close | merged | ✅ |

### Phase D — Storage & Filesystem (partial) ✅
| Feature | PR | Status |
|---|---|---|
| virtio-blk PCI driver (DMA sector read) | T13 | ✅ |
| SYSFS read-only filesystem parser | T14 | ✅ |
| VFS (mount/open/read/close) | T14 | ✅ |
| File descriptor table | T14 | ✅ |
| UEFI bootloader (GNU-EFI) | boot/ | ✅ |

---

## Current Phase

### P1-5c — SYS_READDIR + ls command
- **Branch**: `origin/p1-5c-ls-readdir` (48 commits ahead of main)
- **Status**: REVIEWING — checking CI status before merge
- **What it adds**:
  - `SYS_READDIR` syscall (directory listing by fd or path)
  - `ls` command in the shell
  - Nested directory support in SimpleFs/VFS
  - Shared SYSFS image generator (`scripts/make-sysfs.py`)
  - Directory extents validation
  - Updated CI workflow with expanded production-boot transcript checks
  - 48 commits with thorough unit tests

---

## Roadmap

### NEXT: P2-1 — Writable VFS + file write syscalls
- `SYS_WRITE` to file descriptors (currently only stdout fd=1)
- `SYS_CREATE` / `SYS_MKDIR`
- In-memory ramfs or simple FAT-style writable FS
- Persist kernel log to file

### P2-2 — Multi-process execution
- Process table (beyond single user process)
- `SYS_SPAWN` or fork/exec primitive
- Per-process file descriptor tables
- Process wait/status collection

### P2-3 — IPC primitives
- Shared memory regions
- Pipes (read/write between processes)
- Signal-like notifications

### P2-4 — SMP foundation
- Per-CPU structures, AP startup (SIPI sequence)
- Spinlock validation across cores
- Per-CPU APIC timer
- Core-safe scheduler

### P3-1 — PS/2 keyboard driver
- 8042 controller init
- Scancode → character mapping
- Integration with shell read path

### P3-2 — Framebuffer display
- UEFI framebuffer from BootInfo
- Pixel rendering, text mode over framebuffer
- VGA-style 80×25 text console

### P3-3 — Native shell improvements
- Port shell from assembly to Rust (compiled ELF)
- More commands: mkdir, rm, cp, mv, ps
- Environment variables

### P4-1 — Network stack
- E1000 / virtio-net driver
- ARP, IPv4, UDP, TCP
- DNS resolver

### P5-1 — GUI / Window manager
- Framebuffer compositor
- Window drawing primitives
- Keyboard/mouse event dispatch

---

## Known Issues / Blockers
- SMP deferred until A-F validated
- VFS is read-only (SYSFS v1 has no write path)
- Shell is hand-written assembly (limits complexity)
- Single user process at a time (no process table yet)
- No writable storage (all state lives in RAM)

---

## CI Status (as of HEAD `38e30add`)
| Job | Status |
|---|---|
| kernel-contract | ✅ (host tests + no_std check) |
| uefi-qemu-boot | ✅ (full boot + 25+ marker assertions) |
| memory-protection-faults (NX, RX-write, NMI, MCE, SMAP, SMEP) | ✅ |
| production-boot (with disk, without disk) | ✅ |

---

## Next Action
1. Check GitHub Actions CI on `origin/p1-5c-ls-readdir` tip (`252cd519`)
2. If green → create PR to main, wait for CI on PR, merge
3. If not green → identify failure, fix on that branch, re-push
4. After merge → fetch new HEAD, start P2-1 (writable VFS)

---

## Session Resume Instructions
Read this file. Run:
```
git -C <repo> log --oneline origin/main -5
git -C <repo> branch -r | grep -v HEAD
```
Compare against "Completed Phases" above. Continue from "Next Action".
