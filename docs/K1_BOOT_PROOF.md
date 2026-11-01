# K1 — preuve de démarrage UEFI

## Résultat

Le jalon K1 a réussi dans GitHub Actions sur QEMU/OVMF, sans système Windows hôte.

- Dépôt : `eemntirey/SYSTEMES`
- Branche : `Osreell`
- Commit validé : `a337f7e8c746f0298c4d36540a1471461af6fd98`
- Workflow : [Independent kernel K1 UEFI boot](https://github.com/eemntirey/SYSTEMES/actions/runs/41327729552)
- Artefact : [ISO et trace de démarrage](https://github.com/eemntirey/SYSTEMES/actions/runs/41327729552/artifacts/13092146367)

## Trace obtenue

Le fichier `debug.log` de l'artefact contient les étapes suivantes, dans cet ordre :

1. `SYSTEMES_UEFI_ENTRY_REACHED`
2. Lecture de l'image noyau depuis l'ESP.
3. Réservation et chargement du noyau à 2 MiB.
4. Découverte de GOP et ACPI RSDP.
5. Capture de la carte mémoire UEFI.
6. `SYSTEMES_UEFI_EXIT_BOOT_SERVICES_OK`
7. `SYSTEMES: kernel entry reached`
8. Validation de la carte mémoire, initialisation du framebuffer et découverte ACPI.
9. `SYSTEMES_KERNEL_OK`

La CI vérifie les marqueurs d'entrée UEFI, de sortie des Boot Services, d'entrée noyau et de succès noyau.

## Artefact ISO

Nom : `systemes-k1.iso`

SHA-256 :

```text
a8cd5de5c3fb1086736d51a2b66f0f1949ad6985369131a549581e537091e76f
```

## Ce que cette preuve ne démontre pas

Elle prouve le démarrage du noyau minimal dans l'environnement virtuel QEMU/OVMF. Elle ne prouve pas encore un démarrage sur PC physique, ni un noyau avec mémoire virtuelle propre, gestion des interruptions, ordonnanceur, pilotes, stockage, réseau ou espace utilisateur. Ces éléments restent dans K2 à K6.
