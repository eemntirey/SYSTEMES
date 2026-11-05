# Matrice de suivi SYSTEMES — état réel au 2026-11-05

Cette matrice remplace les annonces de statut non vérifiées. Elle est établie à
partir de :

- des références git réelles (`git log`, `git ls-remote`, arbres comparés) ;
- des workflows CI présents dans `.github/workflows/` ;
- des exécutions locales reproductibles en WSL2 + QEMU/OVMF + OVMF 4M,
  documentées dans les docs de phase ;
- du badge public du workflow `kernel.yml` pour la branche K2.7.

## Légende

| Statut | Sens |
| --- | --- |
| TERMINÉ | Implémenté, fusionné ou vérifié par un test réel sur le commit concerné. |
| EN COURS | Implémenté sur une branche/PR non fusionnée, ou vérification finale en attente. |
| BLOQUÉ | Dépendance technique ou externe empêchant d'avancer. |
| À DÉVELOPPER | Aucun code fonctionnel dans le dépôt à ce jour. |
| À TESTER SUR MATÉRIEL | Code présent mais jamais exécuté sur une machine physique. |

## Photographie du dépôt

| Élément | Valeur vérifiée |
| --- | --- |
| `main` | `0b732fd` — K3.1.2 « Cycle de vie natif des applications » (#34), 275 commits |
| PR #35 (K2.7) | tête `13a002f`, **non fusionnée** dans `main` ; ref de merge `a2de819` présente ; branche `feat/k2-7-preemptive-kernel-scheduler` = 134 commits d'avance sur `main` |
| Branche de cette PR | `feat/k2-8-fpu-simd-context`, empilée sur la tête de #35 |
| Noyau dans `main` | K2.1 allocateur de frames, K2.2 GDT/IDT/exceptions, K2.3 paging noyau |
| Noyau sur la branche K2.7 | + K2.4 W^X/NX, K2.5 TSS/IST, K2.6 ACPI/APIC/IOAPIC/PS/2, K2.7 scheduler |
| CI | `ci.yml` (typecheck/tests/build web + hôte natif Windows), `kernel.yml` (contrat noyau, boot UEFI QEMU, matrice de 8 scénarios matériels après cette PR) |
| Accès GitHub authentifié | **indisponible dans cet environnement** : `api.github.com` renvoie 403 (quota non authentifié) et les pages HTML sont tronquées. Les états de review/checks par commit ne sont donc pas vérifiables ici. |

### Pourquoi K2.7 n'apparaît pas dans `main`

Le graphe de commits montre que K2.4 → K2.7 ont été développés sur une branche
longue durée et que les PR intermédiaires (#22, #28, #30, #33) ont été fusionnées
**dans cette branche**, pas dans `main`. La PR #35 porte donc l'intégralité de
K2.4–K2.7. Toute nouvelle phase noyau doit s'empiler sur #35 tant qu'elle n'est
pas fusionnée, sans quoi les dépendances K2.4–K2.7 seraient cassées.

## A. Noyau (kernel)

| Tâche | Statut | Preuve / remarque |
| --- | --- | --- |
| K1 — loader UEFI GNU-EFI, contrat `BootInfo`, GOP, ACPI RSDP | TERMINÉ | `docs/K1_BOOT_PROOF.md`, job `uefi-qemu-boot` |
| K2.1 — allocateur de frames physique | TERMINÉ | `kernel/src/memory.rs`, tests hôtes |
| K2.2 — GDT, IDT, exceptions CPU | TERMINÉ | `kernel/src/interrupts.rs` |
| K2.3 — paging noyau 4 niveaux | TERMINÉ | `kernel/src/paging.rs` |
| K2.4 — NX, W^X, fautes mémoire réelles | TERMINÉ (branche) | scénarios QEMU `qemu-nx-execute-test`, `qemu-rx-write-test` |
| K2.5 — TSS, piles IST, routage IDT | TERMINÉ (branche) | scénarios `qemu-ist-nmi-test`, `qemu-ist-machine-check-test` |
| K2.6.1 — découverte/validation ACPI MADT | TERMINÉ (branche) | `kernel/src/acpi.rs`, tests hôtes |
| K2.6.2 — xAPIC/IOAPIC, routage IRQ0/PIT | TERMINÉ (branche) | scénario `qemu-pit-timer-test` |
| K2.6.3 — IRQ spurious et dispatch matériel | TERMINÉ (branche) | scénario `qemu-apic-spurious-test` |
| K2.6.4 — PS/2 8042, décodeurs, IRQ1/IRQ12, E2E QMP | TERMINÉ (branche) | `kernel-ps2-input.yml`, PR #28/#30/#33 |
| K2.7 — scheduler préemptif mono-cœur | EN COURS | PR #35 ouverte, non fusionnée ; badge `kernel.yml` de la branche = passing |
| K2.8 — contexte FPU/SIMD par tâche | EN COURS | cette PR ; QEMU/OVMF local vert + contrôle négatif |
| K2.9 — calibration du timer Local APIC | À DÉVELOPPER | la valeur `250 000` est provisoire (documentée en K2.7) |
| SMP, démarrage des AP, IPI, synchronisation inter-processeur | À DÉVELOPPER | aucun code |
| Heap noyau (allocateur dynamique) | À DÉVELOPPER | seules les frames physiques existent |
| Appels système et Ring 3 | À DÉVELOPPER | aucune entrée syscall, aucune GDT utilisateur |
| Stabilisation des exceptions et récupération | EN COURS | exceptions fatales uniquement ; pas de reprise |
| Gestion des ressources du noyau | À DÉVELOPPER | pas de comptabilité mémoire/objets |

## B. Processus et multitâche

| Tâche | Statut |
| --- | --- |
| Tâches noyau préemptives (ring 0) | EN COURS (K2.7) |
| Création/destruction dynamique de tâches | À DÉVELOPPER (table statique de 8) |
| Threads indépendants | À DÉVELOPPER |
| Espaces mémoire séparés | À DÉVELOPPER |
| Mode utilisateur Ring 3 | À DÉVELOPPER |
| IPC, signaux, événements | À DÉVELOPPER |
| Ordonnancement multicœur | À DÉVELOPPER |
| Priorités, zombies, limites de ressources | À DÉVELOPPER |

## C. Mémoire

| Tâche | Statut |
| --- | --- |
| Allocateur physique | TERMINÉ |
| Mémoire virtuelle noyau (identité 512 Gio) | TERMINÉ |
| Heap noyau et utilisateur | À DÉVELOPPER |
| Copy-on-write, mémoire partagée, mmap | À DÉVELOPPER |
| Protection des espaces mémoire | PARTIEL : W^X/NX noyau uniquement |
| Détection de fuites, OOM | À DÉVELOPPER |

## D. Pilotes matériels

| Tâche | Statut |
| --- | --- |
| ACPI | PARTIEL : RSDP + MADT uniquement (pas de FADT/DSDT/AML) |
| PIC/PIT, xAPIC/IOAPIC | TERMINÉ (branche) |
| PS/2 clavier/souris | TERMINÉ (branche) |
| Framebuffer GOP | PARTIEL : marqueur de boot, pas de console ni compositeur |
| PCI/PCIe, enumeration | À DÉVELOPPER |
| USB xHCI, HID, mass storage | À DÉVELOPPER |
| Audio | À DÉVELOPPER |
| SATA/AHCI, NVMe | À DÉVELOPPER |
| Ethernet, Wi-Fi, Bluetooth | À DÉVELOPPER |
| Batterie, thermique, énergie | À DÉVELOPPER |

## E. Stockage et fichiers

| Tâche | Statut |
| --- | --- |
| VFS | À DÉVELOPPER |
| FAT32 (lecture/écriture), partition EFI | À DÉVELOPPER côté noyau (l'ISO de test est fabriquée par `mkfs.vfat` en CI) |
| Système de fichiers natif, journalisation, récupération | À DÉVELOPPER |
| Permissions, montage/démontage, cache disque | À DÉVELOPPER |

## F. Sécurité

| Tâche | Statut |
| --- | --- |
| Isolation processus / Ring 3 | À DÉVELOPPER |
| W^X, NX, write-protect superviseur | TERMINÉ (branche) |
| Comptes, permissions, administrateur | À DÉVELOPPER |
| ASLR, secrets, audit | À DÉVELOPPER |
| Secure Boot, mises à jour signées | À DÉVELOPPER (l'ISO n'est pas signée) |

## G. Réseau

Ethernet, ARP, IPv4/IPv6, ICMP, UDP/TCP, DNS, DHCP, TLS, Wi-Fi, pare-feu,
diagnostic : **À DÉVELOPPER** dans le noyau autonome. Aucune pile réseau
freestanding n'existe à ce jour.

## H. Interface graphique Obsidian

| Tâche | Statut |
| --- | --- |
| Bureau Obsidian actuel (React/Vite, hôte Windows) | TERMINÉ dans son périmètre hôte — PR #23, refonte du menu Démarrer |
| Serveur graphique natif, compositeur, gestionnaire de fenêtres | À DÉVELOPPER |
| Multi-écrans, thèmes, accessibilité, sessions | À DÉVELOPPER côté autonome |
| Portage/abstraction pour supprimer la dépendance Windows | À DÉVELOPPER |

Le design Obsidian et les fonctionnalités du bureau existant sont préservés :
cette PR ne touche aucun fichier de `src/`, `native/` ou `native-core/`.

## I. Applications natives

| Tâche | Statut |
| --- | --- |
| Fondation « applications natives » K3.1 (hôte Windows) | TERMINÉ — PR #31/#32/#34, `docs/architecture/native-applications-foundation.md` |
| Applications natives autonomes (explorateur, terminal, etc.) | À DÉVELOPPER |
| Navigateur, lecteur multimédia, gestionnaires | À DÉVELOPPER |

## J. Services système

Services hôte Windows (paramètres, mise à jour, services) : TERMINÉ dans le
périmètre Windows (commits `0e23a2e`, `c8d921d`). Services autonomes
(initialisation, journaux, horloge, énergie, utilisateurs, tâches planifiées,
notifications, mises à jour) : **À DÉVELOPPER**.

## K. Installation

| Tâche | Statut |
| --- | --- |
| ISO UEFI amorçable de bout en bout (loader + noyau) | TERMINÉ — QEMU/OVMF, CI et local |
| Installateur, partitionnement, installation sur disque physique | À DÉVELOPPER |
| Premier utilisateur, premier démarrage | À DÉVELOPPER |
| Récupération après panne, mise à niveau, désinstallation | À DÉVELOPPER |

## L. Tests et stabilité

| Tâche | Statut |
| --- | --- |
| Tests unitaires hôtes | TERMINÉ — 62 tests verts sur Linux |
| Vérification de compilation `x86_64-unknown-none` | TERMINÉ |
| Tests d'intégration QEMU/OVMF (exceptions, NX, RX, IST, PIT, APIC, scheduler, FPU) | TERMINÉ sur la branche — 8 scénarios après cette PR ; CI obligatoire |
| Tests de charge, fuzzing, tests de sécurité, régression longue durée | À DÉVELOPPER |
| Tests sur matériel physique réel | À TESTER SUR MATÉRIEL — aucune exécution réelle effectuée |

## M. Écosystème

API système stable, SDK, bibliothèque C, support Rust utilisateur, chargeur ELF
applicatif, gestionnaire de paquets, dépôt d'applications, documentation
développeur : **À DÉVELOPPER**. Le noyau produit déjà un binaire ELF, mais aucun
chargeur d'applications n'existe.

## Blocages et vérifications restantes

1. **Fusion de #35** : impossible à valider ici (pas d'accès API authentifié
   pour lire les checks du commit exact, pas d'autorisation de fusion). Le badge
   public indique « passing » sur la branche, ce qui ne remplace pas la
   vérification du commit exact.
2. **CI de cette PR** : à confirmer par le workflow `kernel.yml` déclenché par la
   PR (le job FPU et le boot standard sont les preuves attendues).
3. **Matériel physique** : jamais testé. UEFI/OVMF en QEMU n'est pas une preuve
   de compatibilité matérielle réelle (ACPI, APIC, PS/2, GOP varient).
4. **Windows n'est pas encore supprimé de l'écosystème** : le bureau et les
   applications « natives » actuels dépendent de Windows/.NET. L'OS autonome
   s'arrête aujourd'hui au noyau ring 0 sans FS, réseau ni Ring 3.

## Prochaines tâches techniquement réalisables

1. Fusionner #35 puis cette PR après CI verte sur le commit exact et autorisation.
2. K2.9 : calibrer le timer Local APIC (PIT/TSC) et exposer une horloge noyau
   monotone en millisecondes.
3. Heap noyau + création/destruction dynamique de tâches sur cette base.
4. Ring 3 : GDT utilisateur, TSS `RSP0`, IDT DPL=3, entrée syscall, pages
   utilisateur et retour d'exception.
5. VFS minimal en RAM puis FAT32 en lecture avant tout pilote de stockage.
6. Pilote PCI + AHCI/NVMe, puis FAT32 persistant.
