# SYSTEMES — feuille de route du noyau indépendant et de l’ISO bootable

## État réel au 1er novembre 2026

Le produit Windows existant est une édition Desktop/Host exécutée sous Windows :
WPF/.NET 8 héberge une interface React via WebView2 et le moteur natif pilote certains
services de l'installation Windows.

Le sous-projet Rust est distinct. Il a maintenant franchi K1 : une image ISO UEFI
démarre dans QEMU/OVMF, le chargeur quitte les Boot Services et le point d'entrée du
noyau autonome est exécuté. Cela constitue une première preuve de démarrage sans
Windows hôte, mais **pas encore un OS généraliste**.

## Architecture

- `kernel/` : noyau Rust `no_std`, cible `x86_64-unknown-none`.
- `boot/` : application de démarrage UEFI x86_64.
- `userland/` : futurs services et applications en espace utilisateur.
- `image/` : scripts de construction des images EFI/ISO.
- `docs/` : contrats d'ABI, décisions d'architecture et validation.
- `src/` et `native/Windver/` : édition Windows actuelle, maintenue séparément.

## Jalons et état

### K0 — socle reproductible

<badge>Validé</badge>

- Contrat de transfert `BootInfo` versionné.
- Crate Rust `no_std`, cible `x86_64-unknown-none`.
- Tests du contrat et vérification de la cible dans GitHub Actions.

### K1 — démarrage UEFI minimal

<badge color="success">Réussi dans QEMU/OVMF</badge>

- Le chargeur UEFI lit `EFI/BOOT/KERNEL.BIN` depuis l'ESP.
- Il réserve l'adresse physique `0x00200000` et charge le binaire noyau.
- Il découvre GOP et ACPI RSDP.
- Il récupère la carte mémoire UEFI, son nombre d'entrées, la taille/version des descripteurs.
- Il appelle `ExitBootServices` et transfère le contrôle à `_start(BootInfo*)`.
- Le noyau valide le contrat, initialise un marqueur framebuffer et émet `SYSTEMES_KERNEL_OK`.
- La CI construit l'ISO puis vérifie les marqueurs de démarrage sous QEMU/OVMF.

Preuve reproductible : [K1_BOOT_PROOF.md](K1_BOOT_PROOF.md).

**Limites actuelles de K1 :** chargement à adresse fixe, hypothèse de mapping UEFI identité,
absence de tables de pages propres, GDT/IDT, gestionnaire d'exceptions, allocateur,
ordonnanceur et pilotes. Le pointeur RSDP est découvert mais les tables ACPI ne sont pas
encore analysées. Le framebuffer ne sert qu'à dessiner un marqueur élémentaire.
Le démarrage sur PC physique n'a pas encore été validé.

### K2 — primitives CPU et mémoire

- Initialiser une GDT et une IDT propres au noyau.
- Installer des gestionnaires d'exceptions avec traces série/debug.
- Lire et valider les descripteurs UEFI et construire un gestionnaire de mémoire physique.
- Créer un allocateur de pages et une mémoire virtuelle contrôlée.
- Critère : tests QEMU des allocations/libérations, exceptions attendues et protections
  de pages, sans dépendre des tables de pages du firmware.

### K3 — temps, interruptions et ordonnanceur

- Initialiser APIC/PIC selon la plateforme et un timer monotone.
- Introduire tâches noyau, changement de contexte et synchronisation.
- Ne pas annoncer SMP avant les tests multicœurs.
- Critère : plusieurs tâches noyau s'exécutent et les interruptions sont traitées sans blocage.

### K4 — ABI système et stockage minimal

- Définir les appels système, transitions user/kernel et codes d'erreur.
- Ajouter un VFS puis un système de fichiers en lecture seule.
- Ajouter un pilote bloc virtio-blk pour QEMU avant le matériel réel.
- Critère : lecture d'un fichier de test depuis un disque virtuel sans dépendance Windows.

### K5 — espace utilisateur et shell

- Charger un programme utilisateur distinct du noyau.
- Fournir console, entrée clavier, gestion de processus et permissions minimales.
- Garder l'interface React actuelle comme référence UX, pas comme dépendance du noyau.
- Critère : un shell utilisateur démarre, exécute une commande et survit à l'arrêt d'un enfant.

### K6 — ISO bootable et validation élargie

- Construire une ISO hybride UEFI avec ESP FAT.
- Tester démarrage à froid, redémarrage, absence de disque et mémoire insuffisante.
- Publier SHA-256, trace de démarrage et captures QEMU avec chaque artefact.
- Ajouter progressivement les essais sur machines physiques, après stabilisation de K2-K5.
- Critère : pipeline vert et démarrage autonome depuis l'ISO, sans Windows hôte.

## Règles d'ingénierie

1. Ne jamais appeler le service .NET actuel « noyau ».
2. Ne pas remplacer les opérations Windows par des succès simulés.
3. Ne pas intégrer de pilote matériel réel avant un équivalent virtuel testé dans QEMU.
4. Ne pas effectuer d'opérations destructrices sur disque pendant les premiers jalons.
5. Chaque jalon doit avoir des tests automatiques, des traces et un artefact vérifiable.
6. L'édition Windows reste fonctionnelle pendant le développement du noyau.

## Hors périmètre du premier démarrage

Wi-Fi/Bluetooth matériel, GPU accéléré, audio, USB universel, ACPI avancé,
gestion d'énergie complète, compatibilité avec tous les PC et exécution des applications
Windows. Ces fonctions nécessitent des pilotes, une ABI et une validation spécifiques.
