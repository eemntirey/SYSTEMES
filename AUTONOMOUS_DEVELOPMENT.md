# SYSTEMES — État actuel et journal de développement

## Référence de vérité

- Dépôt : `eemntirey/SYSTEMES`
- Branche de référence : `main`
- Snapshot inspecté : commit de base `56b5f2147d6c82c4be3ed75a501cc0b1f43f2509` (25 novembre 2026).
- Le code fusionné dans `main` prime sur les journaux historiques, les branches et les PR ouvertes.
- Une capacité n'est déclarée opérationnelle qu'avec code dans `main`, compilation, tests et preuve QEMU/matériel attachée au SHA concerné.

## État livré

- Noyau Rust `no_std` x86-64 et démarrage UEFI/OVMF.
- Mémoire physique/virtuelle, protections NX/W^X, SMEP/SMAP conditionnelles, GDT/IDT/TSS/IST.
- Ordonnanceur préemptif, runqueues par CPU et infrastructure SMP ; les capacités doivent être évaluées par scénario, sans assimiler AP démarré à migration Ring 3.
- Ring 3, chargeur ELF de test intégré, syscalls minimales 0–8, shell UART.
- Cycle de vie Ring 3, sortie non-retournante et réclamation différée livrés par les PR fusionnées jusqu'à #101 ; voir `docs/STATE.md` pour les SHA et preuves.
- Virtio-blk en lecture seule et VFS SYSFS en lecture seule.

## Limites opérationnelles actuelles

- VFS read-only ; FAT32 RW, flush durable et persistance après reboot non démontrés.
- ABI utilisateur limitée aux 9 syscalls documentées ; pas de fork/exec/waitpid, mmap, IPC ou libc livrés.
- ELF de test intégré en mémoire, pas de chargement général depuis le stockage.
- Pilotes PC essentiels, réseau, interface graphique et écosystème applicatif natif manquants.
- Pas de validation sur PC physique ni de test d'endurance 72 h documenté.
- PR #83 (écriture/Cache/tmpfs/devfs) et PR #103 (preuve shootdown distant AP) ne sont pas des livraisons de `main` tant qu'elles restent ouvertes.

## Prochaines priorités

1. Fermer la preuve SMP/TLB sur le SHA final de la PR #103 ; vérifier AP réellement en ligne, ACK distant par AP et compteurs de travail.
2. Prouver la migration des tâches noyau et préserver l'affinité Ring 3 tant que son contrat n'est pas migrable.
3. Compléter le modèle processus/threads et l'ABI utilisateur.
4. Livrer VFS inscriptible, FAT32 RW, flush/fsync et test persistance QEMU après reboot.
5. Ajouter pilotes d'entrée/stockage, réseau, puis environnement graphique.
6. Construire libc/runtime, gestionnaire de paquets, comptes et outils système.
7. Produire une distribution installable et valider du matériel réel.

Roadmap détaillée : `docs/PLAN_PROCHAINE_ETAPE.md`.
Matrice comparative : `SYSTEMES_WINDOWS_FUNCTIONAL_GAP_AUDIT.md`.
Contrat syscall réellement livré : `docs/SYSCALL_ABI.md`.

## Politique de livraison

- Une PR ciblée par work package.
- Aucun merge avant CI entièrement verte sur le SHA exact.
- Préserver les tests de non-régression P0 Ring 3, SMP et TLB.
- Ne jamais annoncer une commande cargo/QEMU exécutée sans terminal et résultat observable.
- QEMU/OVMF et matériel physique sont deux niveaux de preuve distincts.

## Journal historique

Les entrées datées ci-dessous sont conservées pour la traçabilité. Elles décrivent l'état de leur époque et ne remplacent pas la section « État actuel » ci-dessus. En cas de contradiction, le code de `main`, `docs/STATE.md` et les résultats CI du SHA courant font foi.

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
