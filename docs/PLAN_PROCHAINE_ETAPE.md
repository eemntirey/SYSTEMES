# SYSTEMES — plan de développement de l'OS autonome

Ce document décrit exclusivement les livraisons du système d'exploitation autonome. Il ne définit aucune édition Desktop/Host Windows ni aucun produit parallèle.

## Priorité immédiate — terminer et stabiliser le socle noyau

Le socle mémoire A est prioritaire : l'allocateur de frames et le heap dynamique doivent être terminés avant les étapes timer/temps et ordonnanceur. La chaîne historique K2.7 → K2.10 contient des PR empilées sur un historique antérieur à la suppression de l'ancien environnement Windows ; elle ne doit pas être fusionnée directement dans `main` sans portage chirurgical des seuls changements noyau et revalidation.

Ordre de travail :

1. Heap dynamique du noyau (A) — intégré dans `main` par la PR #49.
2. Découverte ACPI/MADT — prérequis matériel du Local APIC, sans activer encore les interruptions APIC.
3. Timer Local APIC calibré et horloge monotone (B).
3. Ordonnanceur préemptif et isolation FPU/SIMD (C), après ses prérequis APIC.
4. Ring 3, SYSCALL/SYSRET et processus (D/E).
5. Shell texte utilisateur (F).

SMP complet reste différé jusqu'à validation des étapes A à F. Pour chaque PR : vérifier le HEAD exact, tous les check-runs et la CI complète.

## Prochaines capacités à livrer

### P1 — mémoire et exécution isolée
- gestionnaire de frames physiques et allocateur de pages extensible ;
- pagination et espaces d'adressage indépendants ;
- démarrage SMP, structures per-CPU, synchronisation et ordonnancement multicœur ;
- passage Ring 3 et retour Ring 0 sécurisé ;
- interface d'appels système SYSCALL/SYSRET ;
- processus, threads, terminaison et IPC.

### P2 — services fondamentaux
- VFS et abstraction des systèmes de fichiers ;
- pilote de stockage virtio-blk, puis pilotes matériels ciblés ;
- entrées clavier/souris et console ;
- gestion des périphériques, interruptions et temporisateurs ;
- pile réseau et premiers pilotes réseau.

### P3 — environnement utilisateur natif
- chargeur de programmes et ABI utilisateur documentée ;
- shell natif et outils système ;
- serveur graphique, framebuffer/compositeur et gestionnaire de fenêtres ;
- gestionnaire de fichiers, paramètres et gestionnaire de tâches natifs ;
- applications natives et gestion des utilisateurs/permissions.

### P4 — installation et exploitation
- installateur EFI/ISO avec confirmation et protection contre l'écrasement involontaire des disques ;
- mise à jour atomique, récupération et diagnostic ;
- compatibilité matérielle élargie, documentation et images reproductibles.

## Validation obligatoire

Chaque livraison doit fournir :
- tests unitaires et tests d'intégration pertinents ;
- compilation de la cible noyau `x86_64-unknown-none` ;
- démarrage QEMU/OVMF avec assertions de marqueurs pour les fonctions concernées ;
- CI entièrement verte sur le SHA exact ;
- limites connues clairement documentées.

Les validations QEMU et matériel physique sont deux niveaux distincts. Ne pas déclarer une fonction matériellement validée sans essai sur machine physique.

## Garde-fous

- Le noyau Rust est le cœur du produit ; aucun hôte Windows, navigateur, WebView ou service .NET ne doit être requis pour démarrer SYSTEMES.
- Ne pas présenter des toggles locaux ou des données de démonstration comme des fonctions OS.
- Préserver les composants du noyau déjà fonctionnels ; changements chirurgicaux et PR ciblées.
- Aucun Kilo Code Review.
- Aucune fusion avant CI entièrement verte.
- Si un agent ne dispose pas d'un terminal, il doit indiquer les commandes de vérification à exécuter par l'utilisateur et ne pas prétendre les avoir lancées.
