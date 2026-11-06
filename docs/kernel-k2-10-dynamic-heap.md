# K2.10 — Heap dynamique du noyau

## Objectif

Fournir un allocateur dynamique freestanding pour le noyau, sans dépendance
Windows/.NET et sans allocation pendant son initialisation.

## Architecture initiale

- Arène statique de 1 Mio réservée dans le BSS et alignée sur 4 Kio.
- Initialisation après activation de la table de pages du noyau et vérification
  des permissions RW/NX.
- Liste libre first-fit, blocs alignés, découpage du bloc lors d'une allocation.
- En-tête d'allocation placé immédiatement avant le pointeur retourné.
- Réinsertion ordonnée et fusion des blocs libres adjacents lors de la libération.
- Allocateur exposé via `GlobalAlloc` pour permettre l'utilisation ultérieure de
  `alloc::Box`, `Vec`, `String` et des structures de services noyau.

## Synchronisation et limites

Le verrou de l'allocateur sauvegarde IF, désactive les interruptions locales
pendant la section critique, puis restaure l'état antérieur. Cela évite qu'une
tâche soit préemptée en tenant le verrou et bloque le CPU mono-cœur.

Cette primitive n'est pas encore un verrou SMP : l'activation multi-cœur devra
remplacer cette synchronisation par une stratégie par CPU ou un spinlock
inter-CPU avec exclusion locale des interruptions.

Le heap initial est borné à 1 Mio, sans extension par pages, sans zones dédiées
DMA et sans garde-page. L'épuisement renvoie un pointeur nul conformément au
contrat `GlobalAlloc`; le noyau devra définir une politique d'échec avant
d'autoriser les allocations dynamiques dans les chemins critiques.

## Tests

Les tests hôtes couvrent l'alignement, l'écriture/lecture, la réutilisation
après libération, le découpage/fusion des blocs, l'arène invalide et l'épuisement.
La CI de boot vérifie le marqueur d'initialisation du heap après activation
des permissions mémoire.

Aucun test matériel physique n'est revendiqué. La CI doit être verte sur le
HEAD exact avant toute fusion.
