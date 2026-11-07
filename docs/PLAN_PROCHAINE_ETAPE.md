# SYSTEMES — plan de développement de l'OS autonome

Ce document décrit exclusivement les livraisons du système d'exploitation autonome. Il ne définit aucune édition Desktop/Host Windows ni aucun produit parallèle.

## Priorité immédiate — consolider le socle noyau

Le socle mémoire, interruptions et ordonnancement est livré dans `main`. Les branches historiques `feat/k2-7-*` à `feat/k2-11-*` sont périmées (empilées sur un historique antérieur à la suppression de l'ancien environnement Windows) et ne doivent pas être fusionnées : leurs capacités utiles ont été portées et revalidées dans des PR ciblées.

Ordre de travail :

1. Heap dynamique du noyau — livré (PR #49).
2. Découverte ACPI/MADT — livrée (PR #50).
3. Timer Local APIC calibré et horloge monotone — livré (PR #51).
4. Ordonnanceur coopératif puis préemptif, isolation FPU/SIMD — livrés (PR #52, #53, #54).
5. TSS/IST et sommeil/réveil des tâches noyau — livrés (PR #57).
6. Hygiène debug (CRLF réels, console factorisée) — livrée (PR #58, #59).
7. Exceptions reprenables (#BP avec `iretq`) — livrée (PR #60).
8. Espaces d'adressage par processus — livrés (PR #61) ; SMEP/SMAP et helpers uaccess — livrés dans cette PR.
9. Ring 3, SYSCALL/SYSRET, processus (D/E) — prochaine capacité.
10. Chargeur de programmes et shell texte utilisateur (F).

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
