# K2.8 — Contexte FPU/SIMD des tâches noyau

## Périmètre

K2.8 rend l'état x87/SSE privé à chaque tâche du noyau et l'intègre au chemin
d'interruption matériel existant. Il s'appuie sur le scheduler préemptif K2.7
(PR #35) : cette PR est donc empilée sur `feat/k2-7-preemptive-kernel-scheduler`
tant que K2.7 n'est pas fusionnée dans `main`.

Le noyau reste freestanding (`x86_64-unknown-none`) et n'introduit aucune
dépendance Windows, WPF, WebView2 ou .NET à l'exécution.

## Décision : FXSAVE64 plutôt que XSAVE

La cible Rust `x86_64-unknown-none` désactive SSE/MMX et utilise `+soft-float`.
Le compilateur n'utilise donc jamais les registres XMM/YMM pour du code Rust
ordinaire : l'état FPU/SIMD ne change que par instructions explicites. Un seul
bloc `FXSAVE64`/`FXRSTOR64` au point d'entrée/sortie des IRQ suffit à rendre cet
état privé à chaque tâche.

`FXSAVE64` couvre :

- le mot de contrôle, le mot d'état et le mot de tags x87 ;
- les huit registres x87 de 80 bits ;
- `MXCSR` ;
- les seize registres `XMM0..XMM15` (variante 64 bits, indispensable pour
  `XMM8..XMM15`).

Il ne couvre pas les états supérieurs AVX/AVX-512 (`YMM`/`ZMM`, `K0..K7`), qui
exigent `XSAVE` avec un `XCR0` configuré. Cette limite est explicite et
n'introduit aucun risque tant que le noyau et les applications n'utilisent pas
AVX : aucune instruction AVX n'est engendrée pour la cible actuelle.

## Activation CPU

`kernel/src/fpu.rs` configure et vérifie les bits de contrôle avant tout usage :

| Registre | Bit | Valeur | Effet |
| --- | --- | --- | --- |
| CR0 | EM (2) | 0 | autorise x87 au lieu de `#NM` |
| CR0 | MP (1) | 1 | suivi x87/WAIT cohérent |
| CR0 | TS (3) | 0 | pas de commutation paresseuse, pas de `#NM` |
| CR0 | NE (5) | 1 | erreurs x87 signalées par `#MF` |
| CR4 | OSFXSR (9) | 1 | active SSE et `FXSAVE`/`FXRSTOR` |
| CR4 | OSXMMEXCPT (10) | 1 | exceptions SIMD via `#XM` |

Après écriture, les deux registres sont relus. Si un bit ne s'est pas appliqué,
le noyau échoue explicitement (`SYSTEMES_KERNEL_FAIL`) au lieu de continuer dans
un état indéterminé.

## Disposition mémoire

Chaque `Task` possède une `FpuArea` de 512 octets, alignée sur 16 octets par le
type (`#[repr(C, align(16))]`), conformément à l'exigence d'alignement de
`FXSAVE64`. Les images sont validées au boot :

- offset 0 : mot de contrôle x87 = `0x037F` ;
- offset 4 : mot de tags abrégé = `0xFF` (pile x87 vide) ;
- offset 24 : `MXCSR` = `0x1F80`.

Ces offsets et valeurs sont couverts par des tests hôtes
(`fpu::tests::area_layout_matches_fxsave64`,
`initial_image_validation_checks_documented_offsets`).

## Cycle de vie

1. `fpu::initialize()` active CR0/CR4 et réinitialise x87/`MXCSR`.
2. `fpu::verify_round_trip()` prouve sur le CPU réel qu'une image `FXSAVE64`
   restitue `XMM0` et `ST(0)` après destruction volontaire de l'état.
3. `scheduler::initialize()` réinitialise l'état FPU, capture l'image du boot,
   la copie dans chaque tâche (aucune tâche n'hérite des registres d'une autre),
   puis publie l'image de la tâche de boot comme image courante.
4. À chaque IRQ matérielle, le prologue assembleur exécute `FXSAVE64` dans
   l'image courante avant tout code Rust.
5. Lors d'un changement de tâche, le scheduler publie l'image de la tâche entrante
   dans `SYSTEMES_NEXT_FPU_AREA`.
6. L'épilogue assembleur consomme puis efface ce slot et exécute `FXRSTOR64`
   juste avant de restaurer les registres généraux et `IRETQ`.

Le slot « next » est effacé à la consommation : sans cela, une IRQ matérielle
ultérieure (PS/2, PIT) rechargerait une image périmée et écraserait l'état
FPU d'une tâche en cours d'exécution.

## Preuves de test

### Tests hôtes (Linux, `cargo test --all-features`)

62 tests passent, dont les 5 nouveaux tests `fpu::tests::*`.

### Boot QEMU/OVMF standard

Le job `uefi-qemu-boot` exige désormais, en plus des marqueurs existants :

- `SYSTEMES: x87/SSE (FXSAVE64) state enabled and round-trip verified`
- `SYSTEMES: per-task FXSAVE64 context images initialized`

Le job `memory-protection-faults` exige ces marqueurs pour le scénario
`qemu-scheduler-test`.

### Test d'intégration `qemu-fpu-context-test`

Trois tâches workers programment chacune un état distinct :

| Tâche | Mot de contrôle x87 | MXCSR | XMM0/XMM8/XMM15 | ST(0) |
| --- | --- | --- | --- | --- |
| A | `0x037F` | `0x1F80` | motifs `0x10/0x30/0x50` | 1.0 |
| B | `0x077F` | `0x3F80` | motifs `0x90/0xB0/0xD0` | 2.0 |
| C | `0x0B7F` | `0x5F80` | motifs `0x21/0x41/0x61` | 0.5 |
| Boot | `0x0F7F` | `0x7F80` | motifs `0x77/0x88/0x99` | 3.0 |

Chaque tâche revérifie son état à chaque itération pendant 24 ticks du timer
Local APIC, puis le noyau vérifie l'état de la tâche de boot après toutes les
préemptions. Le test exige en plus au moins deux changements de contexte
comptabilisés par tâche.

Marqueurs requis :

- `SYSTEMES_TEST: x87/SSE context preserved across preemptive task switches`
- `SYSTEMES_TEST: boot task x87/SSE state restored after worker preemption`
- `SYSTEMES_TEST: three FPU tasks retained distinct control words and XMM registers`

### Contrôle négatif

Pour prouver que le test détecte réellement une régression, un build jetable
avec `FXRSTOR64` neutralisé a été exécuté : il échoue avec
`SYSTEMES_KERNEL_FAIL: x87/SSE state clobbered between task switches`.
Le test positif ne peut donc pas passer par accident.

### Vérifications locales reproductibles (WSL2 + QEMU/OVMF)

Exécutées le 2026-11-05 sur le HEAD de cette branche :

```bash
# tests hôtes + compilation no_std
cargo test --all-features --manifest-path kernel/Cargo.toml
cargo check --all-features --manifest-path kernel/Cargo.toml --target x86_64-unknown-none

# boot standard (sans feature)
# boot scheduler (non-régression K2.7)
#   --features qemu-scheduler-test
# boot FPU (K2.8)
#   --features qemu-fpu-context-test
```

Le boot est réalisé avec l'ISO UEFI produite par les mêmes commandes que
`.github/workflows/kernel.yml` et OVMF 4M. La CI reste la porte de fusion : le
commit exact de la PR doit être vert avant toute fusion.

## Limites restantes

- Pas d'AVX/AVX-512 : `XSAVE`/`XCR0` et une image de taille dynamique restent à
  faire avant d'exécuter du code utilisateur compilé avec AVX.
- Pas de commutation paresseuse (`CR0.TS` + `#NM`) : chaque IRQ paie un
  `FXSAVE64` de 512 octets, acceptable à ce stade mais optimisable.
- Mono-processeur : les slots courant/suivant sont globaux. Le SMP exigera un
  état par CPU.
- Les tâches restent en ring 0 et partagent l'espace d'adressage : K2.8 ne
  change pas l'isolation mémoire, qui relève des phases Ring 3/syscalls.
- L'image FPU augmente la taille statique du noyau de 4 Kio (8 tâches × 512 o).
