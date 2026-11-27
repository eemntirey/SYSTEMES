# SYSTEMES kernel — noyau autonome x86_64

Le noyau est un binaire Rust `no_std`, ciblé sur `x86_64-unknown-none`. Il démarre via le chargeur UEFI et ne dépend d'aucun hôte Windows, navigateur ou runtime externe.

- Cible : `x86_64-unknown-none`
- Entrée : `_start(BootInfo*)`, ABI System V x86_64
- Contrat de démarrage : structure `BootInfo` versionnée, carte mémoire UEFI, framebuffer GOP et adresse ACPI RSDP
- Mémoire physique : analyse des descripteurs UEFI et allocation/libération de frames de 4 Kio
- Heap noyau : allocateur global first-fit de 1 Mio, alignement, libération et coalescence des blocs
- Mémoire virtuelle : tables de pages x86_64 à quatre niveaux, activation NX, CR0.WP et protections RX / R-NX / RW-NX
- Espaces d'adressage : PML4 privé par processus, mapping noyau PML4[0] partagé, pages utilisateur U/S à partir de 512 Gio et bascule CR3 vérifiée
- Protection superviseur : SMEP/SMAP activés si CPUID les expose, avec helpers `stac`/`clac` et `UserAccessGuard` pour des accès user strictement encadrés
- ACPI : découverte RSDP UEFI et analyse MADT (Local APIC, IOAPIC et overrides ISA)
- Temps : timer Local APIC calibré sur PIT channel 2 et horloge monotone 100 Hz
- Ordonnancement SMP : runqueues bornées par CPU, préemption Local APIC et mécanismes de migration/work-stealing pour les tâches noyau ; les tâches Ring 3 restent CPU-affines dans le périmètre validé
- Contexte FPU/SIMD : sauvegarde/restauration x87/MMX/SSE par tâche avec FXSAVE64/FXRSTOR64 (512 octets alignés sur 16 octets)
- Cycle de vie des tâches : états Runnable/Blocked/Exited, sommeil sur horloge monotone et réveil par échéance ou événement
- CPU : GDT noyau/user, IDT, TSS x86_64 chargé dans TR=0x18, RSP0 dédié et piles IST1/IST2/IST3 pour #DF/NMI/#MC
- Exceptions : #BP reprenable ; les fautes #PF/#GP provenant de Ring 3 disposent d'un chemin de récupération testé ; les fautes noyau/CPL0 et #DF restent fatales et tracées
- Diagnostic : module console partagé (port QEMU `0xE9`, formatage hexadécimal/décimal testé hôte), sauts de ligne CRLF réels (la CI rejette les séquences littérales `\r\n`)

## Capacités T11–T14 validées sur `t11-elf-loader`

Le chargeur ELF64 accepte les exécutables statiques x86_64 à segments `PT_LOAD` (jusqu'à huit), vérifie les plages et débordements, l'alignement, les recouvrements, les permissions W^X et l'appartenance de l'entrée à un segment exécutable. Les tests hôte couvrent les images valides, tronquées et malformées. Le noyau charge et lance un petit ELF intégré en Ring 3.

Le shell utilisateur minimal lit et écrit sur l'UART via `SYS_READ` et `SYS_WRITE`. La CI envoie une entrée série QEMU et vérifie les marqueurs d'exécution, de lecture et d'écriture. L'ABI syscall vérifie les adresses de reprise, la pile utilisateur et les permissions, renvoie `-ENOSYS` pour les appels inconnus et nettoie les bits RFLAGS privilégiés. `SYS_YIELD` cède le CPU à l'ordonnanceur noyau lorsqu'une autre tâche est exécutable. `SYS_EXIT` reprend le contexte noyau, restaure l'espace d'adressage noyau et libère les pages utilisateur et les tables de pages du processus. Voir [docs/SYSCALL_ABI.md](../docs/SYSCALL_ABI.md) pour le contrat des registres, appels et erreurs.

Le pilote virtio-blk effectue une lecture DMA synchrone d'un secteur depuis un périphérique PCI virtio-blk legacy/transitional en QEMU. Le test monte ensuite l'image SYSFS sur le VFS en lecture seule et vérifie `mount/open/read/close` sur le contenu lu du disque. Ces scénarios sont exécutés dans le job CI QEMU/OVMF, séparément des tests hôte.

Au démarrage, le noyau valide le contrat UEFI, initialise l'allocateur de frames, installe sa GDT, son TSS et son IDT, charge TR=0x18, vérifie RSP0 et le routage IST, puis construit et active ses propres tables de pages et applique les protections W^X. La CI valide également les violations NX/RX, les protections SMEP/SMAP lorsque le CPU les expose, ainsi que les commutations IST2/IST3.

## Limites actuelles

T11–T14 constituent un bring-up minimal, pas un OS généraliste : l'ELF de test est intégré en mémoire et n'est pas chargé depuis le VFS ; le shell ne fournit pas encore un interpréteur de commandes. SYSFS est un format personnalisé en lecture seule, et l'intégration VFS testée reste dans le noyau. Le pilote de stockage ne prend en charge que le transport PCI legacy/transitional à BAR I/O, avec une seule file et des lectures synchrones ; le transport virtio moderne et les disques physiques ne sont pas validés. `SYS_READ`/`SYS_WRITE` ne couvrent que l'UART. Les entrées SYSCALL/SYSRET et les états Ring 3 ont été isolés par CPU/tâche dans les livraisons P0 ; cela ne constitue pas une ABI POSIX complète ni une preuve de migration des tâches Ring 3 entre CPU.

Le heap reste limité à une arène statique de 1 Mio. La MADT consomme les enregistrements Local APIC xAPIC, IOAPIC et Interrupt Source Override ; x2APIC n'est pas encore livré. Le timer Local APIC est calibré sur PIT channel 2 et l'horloge monotone avance à 100 Hz. Les tâches noyau partagent l'espace noyau et les pools scheduler sont statiquement bornés (8 slots par CPU). Les tâches Ring 3 sont CPU-affines dans le modèle livré ; leur migration inter-CPU n'est pas revendiquée. La réclamation différée des espaces d'adressage utilise des pins, un masque de CPU et un protocole IPI/ACK documenté dans `docs/STATE.md`. La PR #103 est fusionnée sous `0eea6f4` ; la CI #575 (14/14) valide le scénario QEMU 1/2/4 CPU, avec les AP en ligne et un ACK de shootdown distant sur 2/4 CPU. La preuve couvre l'IPI, le rechargement CR3 et l'ACK de chaque AP cible ; les tâches Ring 3 restent toutefois CPU-affines au BSP, de sorte qu'une traduction d'un espace utilisateur réellement actif sur un AP n'est pas encore testée. Les syscalls documentées sont uniquement les numéros 0 à 8 ; elles ne constituent pas une ABI POSIX complète. Les fautes #PF/#GP CPL3 ont un chemin de récupération ; les fautes noyau/CPL0 restent fatales. Les scénarios `int 2` et `int 18` valident les portes IDT et le mécanisme IST, sans remplacer les essais matériels réels de NMI/Machine Check.

Un démarrage réussi sous QEMU/OVMF ne prouve pas la compatibilité avec un PC physique. La validation matérielle doit être consignée séparément.

## Vérifications locales

```sh
rustup target add x86_64-unknown-none
cargo test --all-features --manifest-path kernel/Cargo.toml
cargo check --all-features --manifest-path kernel/Cargo.toml --target x86_64-unknown-none
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld
```

La chaîne reproductible de construction UEFI/ISO et les tests QEMU/OVMF sont définis dans `.github/workflows/kernel.yml`.
