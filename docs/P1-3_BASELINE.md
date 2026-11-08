# P1-3 — baseline QEMU sur main

Date de référence : 2026-11-08  
Dépôt : `eemntirey/SYSTEMES`  
Commit de référence : `3be337c7efb0509066077c37aefb51f551cac8db`  
Run GitHub Actions : [42481255706](https://github.com/eemntirey/SYSTEMES/actions/runs/42481255706)

## État de la CI de référence

Les 10 check-runs du commit de référence sont terminés avec la conclusion `success` :
- Build UEFI et démarrage QEMU/OVMF
- Boot de production avec SYSFS/virtio-blk
- Boot de production sans disque
- Tests matériels QEMU NX, RX, IST2, IST3, SMEP et SMAP
- Tests de contrat `no_std` et cible `x86_64-unknown-none`

## Démarrage normal — avec SYSFS

Job : `128733469323` — `Production boot — userland shell with SYSFS disk`

Extrait du journal série/debug capturé par le job :

```text
SYSTEMES_KERNEL_OK
SYSTEMES: serial UART initialized
SYSTEMES: SYSFS root mounted from virtio-blk, sectors=3
SYSTEMES: launching userland shell
SYSTEMES_TEST: user shell address space activated
SYSTEMES: userland shell ready
> ZZ ZZ ZZ ZZ ZZ
```

## Démarrage normal — sans disque

Job : `128733469429` — `Production boot — userland shell without disk`

Extrait du journal série/debug capturé par le job :

```text
SYSTEMES_KERNEL_OK
SYSTEMES: serial UART initialized
SYSTEMES: no virtio-blk disk found; booting without a root filesystem
SYSTEMES: launching userland shell
SYSTEMES_TEST: user shell address space activated
SYSTEMES: userland shell ready
> ZZ ZZ ZZ ZZ ZZ
```

## Diagnostic de départ

Le noyau atteint bien le lancement du programme utilisateur ELF64 en Ring 3, avec et sans disque. Le shell existant n'est pas un interpréteur de commandes : son code embarqué lit puis réécrit les octets reçus. Le test d'entrée répété `Z` produit `ZZ ZZ ZZ ZZ ZZ`; le défaut d'écho est donc présent à la baseline.

L'ABI existante est documentée dans `docs/SYSCALL_ABI.md` :
- `SYS_WRITE=1` et `SYS_READ=2`, fd série `1`
- `SYS_EXIT=3`, retour à la continuation noyau
- `SYS_YIELD=4`, yield coopératif

## Périmètre P1-3

Le développement doit remplacer le programme d'écho minimal par un véritable shell userland ELF64 Ring 3, sans déplacer le parseur dans le noyau. Il devra inclure :
- lecture caractère par caractère via `SYS_READ` ;
- édition de ligne, écho correct et Backspace ;
- parsing de commande et arguments ;
- commandes intégrées documentées et commande inconnue ;
- sortie propre via `SYS_EXIT` ;
- tests hôte et QEMU du chemin utilisateur réel.

Aucune capacité P1-3 n'est déclarée terminée par ce document. Les extraits ci-dessus sont la baseline, pas une preuve des fonctionnalités futures.
