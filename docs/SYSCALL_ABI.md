# SYSTEMES syscall ABI

## Contrat effectivement livré

Ce document décrit uniquement l'ABI réellement raccordée au dispatcher
`kernel/src/syscall.rs` sur le HEAD audité
`a5c89e5d57bca9f874952d96ab801f98df1c27f1`.

L'ABI x86_64 utilise `RAX` pour le numéro d'appel et la valeur de retour.
Les arguments sont transmis dans `RDI`, `RSI`, `RDX`, `R10`, `R8` et
`R9`, dans cet ordre. `SYSCALL` écrase `RCX` et `R11`. Les flags utilisateur
sont filtrés avant le retour. Les pointeurs doivent désigner une plage mémoire
utilisateur canonique, mappée et avec les permissions requises.

| Numéro | Nom | Arguments | Résultat / état |
|---:|---|---|---|
| 0 | `SYS_GET_API_VERSION` | — | Version ABI : `1` |
| 1 | `SYS_WRITE` | `RDI`: fd (1 = série), `RSI`: buffer, `RDX`: longueur | Nombre d'octets écrits ou errno négatif |
| 2 | `SYS_READ` | `RDI`: fd (1 = série, >= 3 = fichier), `RSI`: buffer, `RDX`: longueur max. | Nombre d'octets lus ; série jusqu'à newline ; fichier 0 à EOF |
| 3 | `SYS_EXIT` | `RDI`: statut | Sortie Ring 3 vers la continuation noyau ; pas de retour SYSRET normal |
| 4 | `SYS_YIELD` | — | `0`; cède au scheduler noyau existant |
| 5 | `SYS_UPTIME` | — | Ticks monotones Local APIC depuis le démarrage (100 Hz) |
| 6 | `SYS_OPEN` | `RDI`: pointeur chemin absolu UTF-8, `RSI`: longueur 1–255 | fd >= 3 ou errno négatif |
| 7 | `SYS_CLOSE` | `RDI`: fd >= 3 | `0` ou errno négatif |
| 8 | `SYS_READDIR` | `RDI`: fd répertoire >= 3 ou pointeur chemin NUL-terminé ; `RSI`: buffer utilisateur ; `RDX`: capacité | Octets d'entrées séparées par newline ; 0 à EOF |

## Numéros non livrés — ne pas les utiliser

Les noms suivants ont été mentionnés dans le message du commit du 12 novembre
2026, mais ne sont **pas** définis dans `syscall.rs`, ne sont pas dispatchés
et ne possèdent aucun numéro ABI attribué sur le HEAD audité :

- `SYS_SPAWN`
- `SYS_FORK`
- `SYS_GETPID`
- `SYS_WAITPID`
- `SYS_READKEY`

Ils ne font donc pas partie de l'ABI version 1. Aucun numéro ne doit être
deviné ou réservé implicitement. Leur numérotation, convention d'arguments,
sémantique de blocage et codes d'erreur devront être spécifiés dans une PR
d'implémentation dédiée, avec tests hôte et QEMU/OVMF.

### État des fichiers préparatoires

- `kernel/src/process.rs` contient une table et des helpers de processus,
  mais `kernel/src/lib.rs` ne déclare pas `pub mod process`. Le fichier est
  donc hors du graphe de compilation Cargo. Son `fork_current()` est un stub
  qui retourne `Err("fork not yet implemented")`; aucun `waitpid` n'existe.
- `kernel/src/keyboard.rs` n'est pas déclaré dans `lib.rs`. De plus,
  `Keyboard::init()` retourne inconditionnellement `Ok("ok")` et
  `Keyboard::poll()` retourne toujours `0`. Ce n'est pas un pilote PS/2.
- Aucun appel syscall ne doit être documenté comme opérationnel avant que la
  constante, la branche du dispatcher, le chemin matériel/noyau et les tests
  correspondants soient présents.

## Sémantique des appels livrés

`SYS_UPTIME` lit le compteur monotone Local APIC utilisé par l'ordonnanceur.
Il ne fournit pas l'heure civile. Convertir en secondes par division entière
`ticks / 100`. Le compteur non signé boucle modulo `2^64`.

`SYS_READ` conserve fd `1` comme console série bloquante pour compatibilité
avec le shell. Les fd >= 3 désignent les fichiers ouverts via `SYS_OPEN` ;
les lectures avancent l'offset du handle et renvoient 0 à EOF. Lire un
répertoire renvoie `EISDIR`. Les fd 0 et 2 sont actuellement invalides.

`SYS_READDIR` accepte un fd de répertoire ou un chemin absolu NUL-terminé
(jusqu'à 255 octets). En mode fd, l'offset du handle avance. En mode chemin,
un handle temporaire est ouvert puis fermé avant le retour. Le buffer de
sortie est validé et reçoit des noms UTF-8 complets séparés par `\n`. Si le
prochain nom complet ne tient pas dans un buffer vide, l'appel renvoie
`EOVERFLOW`.

`SYS_OPEN` copie le chemin dans un buffer noyau borné, exige un chemin absolu
UTF-8 et résout les composants imbriqués. `/` ouvre le répertoire racine.
Les chemins relatifs, composants vides, `.`, `..` et NUL intégré sont rejetés
avec `EINVAL`. Un chemin de plus de 255 octets ou un nom SYSFS trop long
renvoie `ENAMETOOLONG`.

Le système de fichiers SYSFS est chargé depuis virtio-blk au démarrage, validé
puis conservé dans une image mémoire noyau. L'absence ou l'échec du disque
laisse le VFS indisponible sans empêcher le démarrage normal. La table de
descripteurs active décrite par ce contrat est celle du chemin Ring 3
actuellement intégré ; ne pas en déduire une isolation multi-processus.

## Erreurs

Les erreurs sont retournées dans `RAX` sous forme d'errno négatif :

| Valeur | Nom | Signification |
|---:|---|---|
| -5 | `EIO` | Échec entrée/sortie série ou VFS |
| -9 | `EBADF` | Descripteur invalide |
| -14 | `EFAULT` | Buffer utilisateur invalide/non mappé/permissions incorrectes |
| -19 | `ENODEV` | Aucun VFS racine monté |
| -20 | `ENOTDIR` | Cible de `READDIR` non répertoire ou composant non répertoire |
| -21 | `EISDIR` | `READ` demandé sur un répertoire |
| -22 | `EINVAL` | Chemin ou argument invalide |
| -24 | `EMFILE` | Limite de fichiers ouverts atteinte |
| -36 | `ENAMETOOLONG` | Chemin trop long |
| -38 | `ENOSYS` | Numéro syscall inconnu |
| -75 | `EOVERFLOW` | Le prochain nom complet ne tient pas dans le buffer |

## Preuves et limites

Sur le commit audité, les tests hôte du dispatcher couvrent notamment la version
ABI, les flags, les numéros inconnus, yield/exit, le layout de frame et les
pointeurs/descripteurs invalides. La CI vérifie également les chemins Ring 3
existants de lecture/écriture, SYSFS et le boot QEMU.

Aucun de ces tests ne couvre `SPAWN`, `FORK`, `GETPID`, `WAITPID` ou
`READKEY`, car ces appels n'existent pas dans le dispatcher. Voir
[`docs/STATE.md`](STATE.md) pour la matrice détaillée et les critères de tests
manquants.
