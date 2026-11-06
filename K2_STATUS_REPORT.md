# SYSTEMES K2 — État Réel au 6 Novembre 2026

## Résumé Exécutif

Le dépôt contient **deux noyaux K2 distincts** à des stades très différents :

| Emplacement | Stade | État Compilation Cible | Tests Hôte |
|-------------|-------|------------------------|------------|
| `SYSTEMES/kernel/` | **K1+ partiel** (BootInfo, allocateur physique, paging 2MiB identity, GDT/IDT basiques) | ✅ OK | ❌ Assembly GNU incompatible |
| `_wt/negctl/kernel/` | **K2.11 avancé** (Paging 4KiB W^X/NX, TSS/IST, APIC/IOAPIC, MADT, scheduler préemptif, FPU/SIMD, PS/2) | ✅ OK (warnings) | ❌ Assembly GNU incompatible |

**Le noyau réel et complet est dans `_wt/negctl/kernel/`.** Le `SYSTEMES/kernel/` est obsolète (K1 avec débuts K2).

---

## Jalons K2 — État Détaillé

| Jalon | Description | `_wt/negctl/kernel` | `SYSTEMES/kernel` |
|-------|-------------|---------------------|-------------------|
| **K2.0** | Contrat BootInfo versionné | ✅ v2 | ✅ v2 |
| **K2.1** | Point d'entrée `_start`, validation handoff | ✅ | ✅ |
| **K2.2** | Allocateur de trames physiques (UEFI map) | ✅ | ✅ |
| **K2.3** | Paging 4 niveaux identity map | ✅ 4KiB + W^X/NX/CR0.WP | ⚠️ 2MiB only, no NX/WP |
| **K2.4** | GDT/IDT + exceptions 0..31 | ✅ TSS + IST (DF/NMI/MC) | ⚠️ Basique, pas de TSS/IST |
| **K2.5** | Interruptions matériel (PIC/APIC) | ✅ PIC remappé + xAPIC + IOAPIC + PIT 100Hz | ❌ |
| **K2.6** | ACPI RSDP + MADT parsing | ✅ Local APIC, IOAPIC, ISO | ❌ |
| **K2.7** | Ordonnanceur préemptif monocœur | ✅ Round-robin, 8 tasks, timer LAPIC | ❌ |
| **K2.8** | Contexte FPU/SIMD par tâche | ✅ FXSAVE64/FXRSTOR64, CR0/CR4, round-trip | ❌ |
| **K2.9** | Clavier/souris PS/2 | ✅ 8042 + décodeurs + files SPSC + IRQ | ❌ |
| **K2.10** | Heap noyau | ❌ Non visible dans les sources | ❌ |
| **K2.11** | Inventaire SMP (MADT) | ✅ Parsing done, **AP startup manquant** | ❌ |

**Légende :** ✅ Implémenté et compilé pour cible | ⚠️ Partiel/obsolète | ❌ Absent

---

## Preuves de Démarrage (K1)

- **CI GitHub Actions** : Workflow `kernel.yml` construit l'ISO et teste sous QEMU/OVMF
- **Artefact** : `systemes-k1.iso` (SHA-256: `a8cd5de5c3fb1086736d51a2b66f0f1949ad6985369131a549581e537091e76f`)
- **Trace validée** : `SYSTEMES_UEFI_ENTRY_REACHED` → `SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK` → `SYSTEMES: kernel entry reached` → `SYSTEMES_KERNEL_OK`
- **Limite** : Validé **uniquement** sur QEMU/OVMF. Pas de test sur matériel physique.

---

## Obstacles Identifiés

### 1. Tests Unitaires Inopérants (Les Deux Noyaux)
- L'assembly inline `global_asm!` utilise la syntaxe GNU (`.type`, `@function`) incompatible avec l'assembleur hôte (MSVC/clang-cl sur Windows)
- `cargo test` échoue sur cible hôte ; `--target x86_64-unknown-none` échoue car `std`/`test` indisponibles
- **Conséquence** : Impossible d'exécuter les tests unitaires en CI locale sans `cargo test --target x86_64-unknown-none -Zbuild-std` ou refactorisation de l'assembly

### 2. Divergence des Deux Noyaux
- `SYSTEMES/kernel/` (main) ≪ `_wt/negctl/kernel/` (worktree)
- Le worktree `_wt` contient le vrai K2 complet ; le main est à l'abandon
- **Action requise** : Synchroniser `SYSTEMES/kernel/` ← `_wt/negctl/kernel/`

### 3. K2.10 (Heap Noyau) Manquant
- Aucun allocateur `alloc::alloc::GlobalAlloc` ni `LinkedListAllocator` / `BuddyAllocator` visible
- Requis pour : structures dynamiques (VFS, gestion processus, piles utilisateur, etc.)

### 4. K2.11 (SMP Bringup) Incomplet
- MADT parsing découvert les Local APICs, mais **pas de code de démarrage des AP** (INIT-SIPI-SIPI)
- Pas de verrouillage multicœur (spinlocks ticket, per-CPU data)

### 5. Absence de Pilotes Matériels (P2)
- Stockage : aucun (pas de virtio-blk, AHCI, NVMe)
- Réseau : aucun
- GPU : framebuffer GOP seulement, pas de pilote DRM/KMS
- USB : aucun

### 6. Espace Utilisateur Absent (P3/P4)
- Pas d'ABI syscall, pas de VFS, pas de FS, pas de chargeur ELF utilisateur
- Pas de shell, pas d'interface graphique native

---

## Prochain Jalon Techniquement Nécessaire : **K2.10 — Heap Noyau**

### Justification
Toutes les fondations (mémoire, paging, interruptions, scheduler, FPU, PS/2) sont en place dans `_wt/negctl/kernel/`. L'heap est le **prérequis indispensable** pour :
- Allouer dynamiquement des `Task` (au-delà du tableau statique de 8)
- Implémenter un VFS avec inodes/dentries dynamiques
- Charger des programmes utilisateur (ELF parser, piles, arguments)
- Gérer des ressources noyau de durée de vie variable

### Spécification Minimale K2.10
1. **Buddy Allocator** ou **Linked List Allocator** sur les trames physiques existantes
2. **`GlobalAlloc` implémenté** pour `alloc::alloc` + `alloc::collections`
3. **Guard pages** et **W^X** sur les allocations heap (réutiliser `paging::protect_regions`)
4. **Tests** : allocation/libération, fragmentation, exhaustion, concurrence (spinlock)
5. **Intégration** : remplacer `MAX_TASKS` statique par allocation dynamique dans le scheduler

### Dépendances
- `PhysicalFrameAllocator` ✅ (déjà exporté)
- `paging::protect_regions` / `map_mmio_uncached` ✅
- `SpinMutex` ✅ (dans scheduler)

---

## Plan d'Action Immédiat

1. **Synchroniser** : Copier `_wt/negctl/kernel/` → `SYSTEMES/kernel/` (remplacer l'ancien)
2. **Corriger l'assembly** : Remplacer `.type ... @function` par syntaxe compatible ou isoler l'assembly dans un `.s` séparé pour permettre `cargo test` hôte
3. **Implémenter K2.10** : Heap noyau (buddy allocator + GlobalAlloc)
4. **Activer CI tests** : `cargo test --target x86_64-unknown-none -Zbuild-std=core,alloc` (nécessite nightly)
5. **Valider QEMU** : Build ISO avec le noyau synchronisé + test démarrage + test scheduler/FPU/PS/2

---

## Note sur K3 (Windows Desktop)

Le répertoire `SYSTEMES/src/`, `SYSTEMES/native/` contient l'édition Windows (React + .NET 8 + WebView2). **Elle est totalement indépendante du noyau K2** et doit le rester. Aucune dépendance du boot K2 vers K3.