# SYSTEMES — feuille de route du noyau indépendant et de l’ISO bootable

## État réel au 1er novembre 2026

Le produit actuel est un environnement de bureau Windows exécuté comme application WPF/.NET 8 avec une interface React hébergée dans WebView2. Le moteur natif interroge et pilote certains services de l’installation Windows hôte.

**Ce n’est pas encore un système d’exploitation autonome.** Le moteur .NET, le shell WPF et les API Win32 ne peuvent pas être présentés comme un noyau démarrable. Ils dépendent de Windows et ne sont pas exécutables au démarrage UEFI sans Windows.

Le développement du noyau doit donc être isolé dans un nouveau sous-projet. Les composants Windows actuels restent disponibles comme édition Desktop/Host et ne sont pas supprimés.

## Objectif initial vérifiable

Produire une image ISO x86_64 UEFI qui démarre dans QEMU/OVMF et affiche un écran de démarrage SYSTEMES depuis un noyau autonome. Le premier jalon ne promet ni compatibilité matérielle générale, ni réseau, ni bureau graphique complet.

## Architecture cible

- 'kernel/' : noyau autonome 'no_std', cible 'x86_64-unknown-none', sans dépendance au runtime Windows.
- 'boot/' : chargeur UEFI x86_64, découverte du noyau, carte mémoire UEFI et transfert contrôlé du contrôle au noyau.
- 'userland/' : futurs services et applications en espace utilisateur, séparés du noyau.
- 'image/' : scripts reproductibles de construction de l’image EFI/ISO et configuration OVMF.
- 'docs/' : contrats d’ABI, décisions d’architecture et procédures de validation.
- 'src/' et 'native/Windver/' : édition Windows actuelle, maintenue séparément.

## Jalons et critères d’acceptation

### K0 — socle reproductible

- Définir explicitement la cible x86_64 UEFI et la version de toolchain Rust.
- Ajouter un build déterministe, un format de logs série et des tests de validation des artefacts.
- Documenter les prérequis QEMU, OVMF et outils ISO.
- Critère : le pipeline produit un artefact EFI vérifiable et échoue si l’artefact est absent ou vide.

### K1 — démarrage UEFI minimal

- Charger le binaire noyau depuis une partition EFI FAT.
- Récupérer la carte mémoire UEFI, les informations framebuffer et la table ACPI avant 'ExitBootServices'.
- Valider la clé de carte mémoire et effectuer 'ExitBootServices' avec gestion de l’échec/retry.
- Transférer le contrôle au point d’entrée du noyau avec un contrat de boot documenté.
- Critère : QEMU/OVMF démarre l’image et le noyau écrit un marqueur de démarrage sur le port série.

### K2 — primitives CPU et mémoire

- Initialiser GDT, IDT, gestion des exceptions et interruptions.
- Détecter la mémoire physique et mettre en place un allocateur de pages.
- Créer un allocateur noyau et un gestionnaire de mémoire virtuelle avec protections.
- Critère : tests QEMU couvrent les allocations/libérations, les pages invalides et les exceptions attendues.

### K3 — temps, interruptions et ordonnanceur

- Initialiser APIC/PIC selon la plateforme, un timer monotone et la gestion d’interruptions.
- Introduire les tâches noyau, le changement de contexte et les primitives de synchronisation.
- Ne pas annoncer le SMP tant que les tests multicœurs ne sont pas disponibles.
- Critère : plusieurs tâches noyau s’exécutent et les interruptions sont traitées sans blocage.

### K4 — ABI système et stockage minimal

- Définir les appels système, les transitions user/kernel et les codes d’erreur.
- Ajouter un VFS, puis un premier système de fichiers en lecture seule avant toute écriture.
- Ajouter un pilote bloc virtio-blk pour QEMU avant de viser du matériel réel.
- Critère : le noyau lit un fichier de test depuis un disque virtuel sans dépendance Windows.

### K5 — espace utilisateur et shell

- Charger un premier programme utilisateur séparé du noyau.
- Fournir console, entrée clavier, gestion de processus et permissions minimales.
- Garder l’interface React actuelle comme référence UX, pas comme dépendance du noyau.
- Critère : un shell utilisateur démarre, exécute une commande et survit à l’arrêt d’un processus enfant.

### K6 — ISO bootable et validation

- Construire une image ISO hybride UEFI avec une partition EFI FAT et les fichiers de boot nécessaires.
- Démarrer en QEMU/OVMF en mode UEFI, avec mémoire et disque virtuel documentés.
- Ajouter des tests de démarrage à froid, redémarrage, absence de disque, mémoire insuffisante et arrêt propre.
- Publier SHA-256, logs série et captures QEMU avec chaque artefact.
- Critère : pipeline vert et preuve d’un démarrage autonome depuis l’ISO, sans Windows hôte.

## Règles d’ingénierie

1. Ne jamais appeler « noyau » le service .NET actuel ni « ISO autonome » un installateur de l’application Windows.
2. Ne pas remplacer les APIs Windows actuelles par des stubs qui renvoient des succès simulés.
3. Ne pas intégrer de pilote matériel réel avant un équivalent virtuel testé dans QEMU.
4. Ne pas ajouter d’opérations destructrices sur disque pendant les premiers jalons.
5. Chaque jalon doit avoir des tests automatiques, des logs et un artefact vérifiable.
6. L’édition Windows reste fonctionnelle pendant la construction du noyau indépendant.

## Hors périmètre du premier ISO

Wi-Fi/Bluetooth matériel, GPU accéléré, audio, USB universel, ACPI avancé, gestion d’énergie complète, compatibilité avec tous les PC et exécution des applications Windows. Ces fonctions exigent des pilotes, une ABI et une validation spécifiques ; elles ne seront pas simulées pour donner l’illusion d’un OS complet.
