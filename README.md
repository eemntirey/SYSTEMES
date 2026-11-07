# SYSTEMES — système d’exploitation autonome

SYSTEMES est un projet de système d’exploitation indépendant, libre d’utilisation et destiné à démarrer directement sur un ordinateur compatible, sans dépendre de Windows.

## Objectif

Construire un OS x86_64/UEFI complet, avec ses propres composants :

- noyau autonome Rust `no_std` ;
- gestion de la mémoire physique et virtuelle ;
- interruptions, horloge, ordonnanceur et multitâche ;
- espaces utilisateur, processus, appels système et IPC ;
- pilotes matériels et gestion des périphériques ;
- VFS et systèmes de fichiers persistants ;
- pile réseau ;
- interface graphique native, gestionnaire de fenêtres et applications ;
- comptes, permissions, installation, mises à jour et récupération.

**Le livrable final est un OS amorçable directement depuis un support EFI/ISO sur un PC physique.** Une interface exécutée dans un navigateur ou hébergée par Windows ne constitue pas SYSTEMES OS.

## Architecture du système

```
boot/       Chargeur UEFI x86_64
kernel/     Noyau Rust no_std
userland/   Services et applications en espace utilisateur
image/      Construction des images EFI/ISO
docs/       Architecture, contrats et preuves de validation
```

Les anciens composants d'interface et d'intégration Windows présents dans l'historique du dépôt ne constituent ni une édition du produit, ni une cible de livraison. Ils ne doivent pas être utilisés comme substitut au noyau, aux pilotes ou à l'interface native de SYSTEMES OS.

## État et validation

Le noyau dispose d'une chaîne de démarrage UEFI validée sous QEMU/OVMF. Le développement des primitives CPU, mémoire, interruptions et multitâche se poursuit.

Les niveaux de validation doivent toujours être distingués :

1. compilé ;
2. tests unitaires/hôtes réussis ;
3. démarrage et tests QEMU/OVMF réussis ;
4. démarrage validé sur matériel physique.

Un test QEMU ne constitue pas une preuve de compatibilité matérielle générale. Ne jamais annoncer une fonctionnalité comme terminée sans preuve adaptée.

## Développement

Les contributions doivent viser directement les composants de l'OS autonome. Chaque livraison doit apporter une capacité vérifiable, des tests automatisés et une documentation concise.

- Aucun composant simulé ne doit être présenté comme matériel ou service réel.
- Aucun test ne doit être déclaré exécuté sans l'avoir réellement lancé.
- Aucune fusion avant une CI entièrement verte sur le HEAD exact.
- Aucune revue Kilo Code.
- Respecter les licences des composants tiers.

## Gratuité

SYSTEMES doit rester gratuit à utiliser, sans activation payante, abonnement ou verrouillage des fonctions essentielles. Les licences des dépendances tierces restent applicables.

## Documentation

- [Feuille de route du noyau et de l'ISO bootable](docs/BOOTABLE_KERNEL_ISO_ROADMAP.md)
- [Programme d'accélération](https://github.com/eemntirey/SYSTEMES/issues/45)
