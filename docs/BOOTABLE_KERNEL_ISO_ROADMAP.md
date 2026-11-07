# SYSTEMES — feuille de route du noyau autonome et de l'ISO bootable

## Mission

SYSTEMES est un seul système d'exploitation autonome x86_64/UEFI. Il doit démarrer directement depuis une image EFI/ISO sur un ordinateur compatible, sans Windows, navigateur, WebView ou hôte externe.

Les anciens composants Windows du dépôt ne sont pas une édition du produit et ne font pas partie de l'architecture cible. Le développement et les validations décrits ici concernent le noyau autonome et son futur espace utilisateur.

## Architecture

- `boot/` : chargeur UEFI x86_64.
- `kernel/` : noyau Rust `no_std`, cible `x86_64-unknown-none`.
- `userland/` : futurs services et applications en espace utilisateur.
- `image/` : scripts de construction des images EFI/ISO.
- `docs/` : contrats d'ABI, décisions d'architecture et preuves de validation.

## État du démarrage

La chaîne UEFI démarre sous QEMU/OVMF : le chargeur lit le noyau depuis l'ESP, récupère les informations de démarrage, quitte les Boot Services et transfère le contrôle au noyau. Cela prouve un démarrage initial sous machine virtuelle, mais ne constitue pas encore un OS généraliste ni une validation sur PC physique.

## Jalons

### K0 — socle reproductible
- Contrat de transfert BootInfo versionné.
- Crate Rust no_std et cible x86_64-unknown-none.
- Tests du contrat et vérification de la cible dans GitHub Actions.

### K1 — démarrage UEFI minimal
- Chargement du noyau depuis l'ESP.
- Découverte GOP et ACPI RSDP.
- Récupération de la carte mémoire UEFI.
- ExitBootServices et transfert de contrôle au noyau.
- Trace de démarrage vérifiée sous QEMU/OVMF.

### K2 — primitives CPU et mémoire
- GDT, IDT et gestionnaires d'exceptions.
- Gestion mémoire physique et virtuelle.
- Tables de pages propres et protections NX/W^X.
- Interruptions matérielles, PIC/APIC et timer.
- Ordonnanceur préemptif, contexte FPU/SIMD et heap dynamique.
- SMP et synchronisation multicœur à valider séparément.

### K3 — exécution utilisateur
- Espaces d'adressage isolés et Ring 3.
- Transitions de privilège et appels système.
- Chargeur ELF, processus, threads et IPC.
- Shell utilisateur indépendant du noyau.

### K4 — stockage et périphériques
- VFS et système de fichiers persistant.
- Pilote bloc virtio pour QEMU, puis stockage matériel ciblé.
- Modèle de pilotes et gestion des périphériques.
- Clavier, souris, framebuffer et gestion des interruptions.

### K5 — réseau et services système
- Pile réseau et pilotes compatibles avec le matériel ciblé.
- Services système, horloge civile et temporisateurs.
- Gestion des utilisateurs, permissions et configuration.

### K6 — interface graphique native
- Serveur graphique/compositeur natif.
- Gestionnaire de fenêtres et shell graphique.
- Gestionnaire de fichiers, terminal, paramètres et applications natives.

### K7 — installation et validation matérielle
- ISO hybride UEFI reproductible.
- Installateur avec confirmation des opérations disque.
- Mise à jour, récupération et diagnostic.
- Essais sur matériel physique, avec matrice de compatibilité et limites documentées.

## Niveaux de preuve

Pour chaque capacité, indiquer séparément :
1. implémentée ;
2. compilée ;
3. tests automatisés réussis ;
4. validée sous QEMU/OVMF ;
5. validée sur matériel physique.

Ne jamais confondre une compilation avec une fonctionnalité opérationnelle, ni un test QEMU avec une validation sur PC physique.

## Règles d'ingénierie

- Aucun composant Windows ou navigateur requis au démarrage.
- Ne jamais présenter une simulation comme une opération matérielle réelle.
- Tester d'abord les pilotes dans QEMU lorsque c'est possible.
- Aucune opération destructive sur disque sans consentement explicite et mécanisme de récupération.
- Chaque jalon doit avoir des tests, des traces et un artefact vérifiable.
- CI entièrement verte sur le HEAD exact avant toute fusion.

## Hors périmètre des premiers jalons

Wi-Fi/Bluetooth matériel, GPU accéléré, audio, USB universel, ACPI avancé, gestion d'énergie complète et compatibilité avec tous les PC nécessitent des pilotes et validations spécifiques. Ils restent des objectifs progressifs, pas des fonctions déjà disponibles.
