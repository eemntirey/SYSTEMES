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
- Ordonnancement : tâches noyau coopératives via 0x41 et préemption par timer Local APIC 0x40
- Contexte FPU/SIMD : sauvegarde/restauration x87/MMX/SSE par tâche avec FXSAVE64/FXRSTOR64 (512 octets alignés sur 16 octets)
- Cycle de vie des tâches : états Runnable/Blocked/Exited, sommeil sur horloge monotone et réveil par échéance ou événement
- CPU : GDT noyau/user, IDT, TSS x86_64 chargé dans TR=0x18, RSP0 dédié et piles IST1/IST2/IST3 pour #DF/NMI/#MC
- Exceptions : #BP reprenable (le CPU sauvegarde l'adresse suivant `INT3` ; le handler restaure le contexte et exécute `iretq`) ; #PF/#GP/#DF restent fatales et tracées
- Diagnostic : module console partagé (port QEMU `0xE9`, formatage hexadécimal/décimal testé hôte), sauts de ligne CRLF réels (la CI rejette les séquences littérales `\r\n`)

## Capacités présentes dans le code

Au démarrage, le noyau valide le contrat UEFI, initialise l'allocateur de frames, installe sa GDT, son TSS et son IDT, charge TR=0x18, vérifie RSP0 et le routage IST, puis construit et active ses propres tables de pages, applique les protections W^X aux sections du noyau et vérifie les permissions effectives. Il initialise également le heap noyau après activation du mapping RW/NX, analyse la MADT ACPI fournie par le RSDP UEFI et initialise le framebuffer GOP lorsqu'il est disponible.

La CI vérifie le contrat `no_std`, la cible x86_64, la construction du chargeur EFI et de l'ISO, le démarrage sous QEMU/OVMF, l'exécution d'un petit programme en Ring 3, l'accès à sa pile user et son retour contrôlé par un vecteur DPL3 utilisant RSP0, ainsi que les violations NX/RX et les commutations de pile IST2/IST3. Cette interruption `int 0x80` est uniquement une porte de sortie de test, pas une ABI syscall. Les scénarios `int 2` et `int 18` sont des interruptions logicielles qui valident les portes IDT et le mécanisme IST ; ils ne simulent pas un NMI électrique ni une véritable Machine Check.

Le heap initial est limité à une arène statique de 1 Mio ; l'extension dynamique par pages reste hors périmètre. La MADT consomme les enregistrements Local APIC xAPIC, IOAPIC et Interrupt Source Override ; x2APIC et le démarrage des AP ne sont pas livrés. Le timer Local APIC est calibré sur PIT channel 2 et l'horloge monotone avance à 100 Hz. L'ordonnanceur prend en charge yield coopératif, préemption timer, sommeil sur échéance et blocage/réveil événementiel ; les tâches partagent l'espace noyau et le changement de contexte reste mono-cœur. Le contexte x87/MMX/SSE est isolé par tâche via FXSAVE64/FXRSTOR64. Les tests IST2/IST3 ne remplacent pas des tests matériels réels de NMI/Machine Check ; aucun Double Fault matériel réel n'est déclenché. Seul le #BP déclenché par `INT3` est reprenable ; les autres exceptions restent fatales. Le test T9 prouve le transfert vers et depuis Ring 3 sur une seule adresse user mappée, mais aucun ordonnanceur utilisateur, processus durable ni ABI d'appel système n'est fourni. Les tables de pages ne sont pas encore libérées. SMEP/SMAP ne sont activés que si le CPU les expose (la CI utilise `-cpu max` pour les prouver ; le modèle QEMU par défaut ne les fournit pas). Les accès user ne sont pas encore validés par marche de tables ni par des fautes récupérables : un pointeur user non mappé reste fatal jusqu'à T10.

## Limites actuelles

Ce noyau n'est pas encore un système d'exploitation généraliste. Les fondations mono-cœur de mémoire, interruptions, horloge, ordonnanceur préemptif, contexte FPU/SIMD et cycle de vie des tâches sont présentes et testées sous QEMU. T9 ajoute un test de transition Ring 3 limitée ; il manque encore SYSCALL/SYSRET, le chargeur ELF, les processus et l'IPC utilisateur, le VFS, les pilotes de stockage et de réseau, un environnement graphique natif, ainsi que l'installateur et le mécanisme de mise à jour. Ces capacités ne doivent pas être annoncées comme disponibles avant leur implémentation et validation.

Un démarrage réussi sous QEMU/OVMF ne prouve pas la compatibilité avec un PC physique. La validation matérielle doit être consignée séparément.

## Vérifications locales

```sh
rustup target add x86_64-unknown-none
cargo test --all-features --manifest-path kernel/Cargo.toml
cargo check --all-features --manifest-path kernel/Cargo.toml --target x86_64-unknown-none
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld
```

La chaîne reproductible de construction UEFI/ISO et les tests QEMU/OVMF sont définis dans `.github/workflows/kernel.yml`.
