# SYSTEMES kernel — noyau autonome x86_64

Le noyau est un binaire Rust `no_std`, ciblé sur `x86_64-unknown-none`. Il démarre via le chargeur UEFI et ne dépend d'aucun hôte Windows, navigateur ou runtime externe.

- Cible : `x86_64-unknown-none`
- Entrée : `_start(BootInfo*)`, ABI System V x86_64
- Contrat de démarrage : structure `BootInfo` versionnée, carte mémoire UEFI, framebuffer GOP et adresse ACPI RSDP
- Mémoire physique : analyse des descripteurs UEFI et allocation/libération de frames de 4 Kio
- Heap noyau : allocateur global first-fit de 1 Mio, alignement, libération et coalescence des blocs
- Mémoire virtuelle : tables de pages x86_64 à quatre niveaux, activation NX, CR0.WP et protections RX / R-NX / RW-NX
- ACPI : découverte RSDP UEFI et analyse MADT (Local APIC, IOAPIC et overrides ISA)
- Temps : timer Local APIC calibré sur PIT channel 2 et horloge monotone 100 Hz
- Ordonnancement : tâches noyau coopératives, changement de contexte via interruption logicielle 0x41
- CPU : GDT, IDT et gestionnaires d'exceptions CPU 0 à 31
- Diagnostic : port debug QEMU `0xE9`

## Capacités présentes dans le code

Au démarrage, le noyau valide le contrat UEFI, initialise l'allocateur de frames, installe sa GDT et son IDT, construit et active ses propres tables de pages, applique les protections W^X aux sections du noyau et vérifie les permissions effectives. Il initialise également le heap noyau après activation du mapping RW/NX, analyse la MADT ACPI fournie par le RSDP UEFI et initialise le framebuffer GOP lorsqu'il est disponible.

La CI vérifie le contrat `no_std`, la cible x86_64, la construction du chargeur EFI et de l'ISO, le démarrage sous QEMU/OVMF ainsi que les fautes matérielles attendues pour les violations NX et l'écriture dans une page RX.

Le heap initial est limité à une arène statique de 1 Mio ; l'extension dynamique par pages et le verrouillage SMP restent hors périmètre. La MADT ne consomme ici que les enregistrements Local APIC xAPIC, IOAPIC et Interrupt Source Override ; les enregistrements x2APIC et le démarrage des AP ne sont pas livrés. Le timer Local APIC est calibré sur PIT channel 2 ; l'horloge monotone avance sur chaque IRQ timer avec une résolution initiale de 10 ms. L'ordonnanceur actuel est coopératif : les tâches partagent l'espace noyau et cèdent explicitement le processeur ; la préemption automatique n'est pas encore livrée.

## Limites actuelles

Ce noyau n'est pas encore un système d'exploitation généraliste. Le code présent ne fournit pas encore un ordonnanceur préemptif intégré à cette branche, le démarrage SMP, Ring 3, les appels système, le chargeur ELF, les processus utilisateur, le VFS, les pilotes de stockage/réseau, ni un environnement graphique natif complet. Ces capacités doivent être implémentées et validées séparément avant d'être annoncées comme disponibles.

Un démarrage réussi sous QEMU/OVMF ne prouve pas la compatibilité avec un PC physique. La validation matérielle doit être consignée séparément.

## Vérifications locales

```sh
rustup target add x86_64-unknown-none
cargo test --all-features --manifest-path kernel/Cargo.toml
cargo check --all-features --manifest-path kernel/Cargo.toml --target x86_64-unknown-none
cargo rustc --manifest-path kernel/Cargo.toml --release --target x86_64-unknown-none --bin systemes-kernel -- -C relocation-model=static -C link-arg=-Tlinker.ld
```

La chaîne reproductible de construction UEFI/ISO et les tests QEMU/OVMF sont définis dans `.github/workflows/kernel.yml`.
