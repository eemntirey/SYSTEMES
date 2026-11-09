# SYSTEMES — journal de développement autonome

## État du dépôt

- **Remote** : https://github.com/eemntirey/SYSTEMES
- **Base de cette branche** : `main` @ `38e30add` (PR #74)
- **Branche** : `p1-5-user-fault-recovery`
- **Dernière mise à jour** : 2026-11-09

## Feuille de route

| Phase | Contenu | État |
|---|---|---|
| A — noyau | GDT/IDT, mémoire physique et virtuelle, heap, NX/W^X, TSS/IST, ACPI, timer APIC | livré |
| B — multitâche | ordonnanceur coopératif puis préemptif, FPU/SIMD, sommeil/réveil | livré (mono-cœur) |
| C — exécution native | espaces d'adressage, Ring 3, SYSCALL/SYSRET, ELF64, shell, syscalls fichier | livré ; **récupération des fautes Ring 3 livrée par cette branche** |
| D — stockage | virtio-blk (legacy/transitional), SYSFS lecture seule, VFS, descripteurs | partiel (lecture seule) |
| E — pilotes | PCI, virtio-blk, clavier/souris, affichage, USB, réseau | à faire |
| F — réseau | Ethernet, ARP, IPv4, UDP/TCP, DNS, services | à faire |
| G — environnement natif | shell, commandes, utilisateurs, permissions, gestionnaire de processus | partiel |
| H — interface graphique | framebuffer, compositeur, fenêtres, bureau | à faire |
| I — installation | UEFI, installateur, partitions, mise à jour, récupération | à faire |
| J — stabilisation | QEMU/OVMF (fait), matériel physique, charge, longue durée | matériel non validé |

## Tâches terminées — cette session

- **P1-4 : récupération des fautes Ring 3** (commit code sur cette branche).
  - Une faute Ring 3 (`#PF`, `#GP`, `#UD`, `#DE`, `#SS`, `#NP`, `#MF`, `#AC`, `#XM`, `#BR`)
    ne panique plus le noyau : le dispatcher ferme la table de fichiers du
    processus, enregistre le vecteur et le statut `128 + vecteur`, réécrit la
    frame d'exception vers la trampoline de sortie noyau, et le code de reprise
    restaure CR3 et libère l'espace d'adressage.
  - Les fautes Ring 0 et `#DF` restent fatales ; `#BP` reste reprenable.
  - `interrupts::user_fault_vector()` expose le vecteur (`NO_USER_FAULT` si le
    processus est sorti par `SYS_EXIT`).
  - Test QEMU `qemu-user-fault-test` : écriture dans une page utilisateur non
    mappée, vérification `vector=0xE`, `cr2`, statut 142, récupération des
    frames, puis lancement d'un second processus qui sort avec `SYS_EXIT`.

## Preuves

- Tests hôte : `cargo test --all-features` → **135/135**.
- Cible noyau : `cargo check --all-features --target x86_64-unknown-none` → OK.
- QEMU/OVMF (build UEFI + ISO + disque SYSFS, jeu de features CI + nouvelle
  feature) : boot complet jusqu'au shell, entrée série réémise, **aucun**
  marqueur `SYSTEMES_KERNEL_FAIL`, et marqueurs de faute présents :
  - `SYSTEMES_EXCEPTION: vector=0x000000000000000E error=0x0000000000000006`
  - `SYSTEMES: killing user process: vector=0x000000000000000E rip=... cr2=0x0000008005000000`
  - `SYSTEMES: user process terminated; kernel resumed`
  - `SYSTEMES_TEST: fault exit status=142 vector=0x000000000000000E`
  - `SYSTEMES_TEST: kernel launched a new user process after the fault`
  - `SYSTEMES_TEST: faulted process frames reclaimed`
- CI : la liste de features et les marqueurs du job `uefi-qemu-boot` incluent
  la nouvelle preuve ; `scripts/qemu-boot.sh --ci-features` la reflète.
  La CI ne peut pas être verte sur cette base : le job combiné de `main`
  échoue déjà avant cette branche. Preuve reproduite sur `origin/main`
  (`38e30add`) sans aucune modification : l'image SYSFS inline du job a une
  racine vide et un fichier de 5 octets alors que `user_fs_test` attend
  `hello
` (6 octets), donc
  `SYSTEMES_KERNEL_FAIL: user SYS_OPEN/READ/CLOSE assertions failed`.
  La PR concurrente p1-5c remplace cette image par `scripts/make-sysfs.py`.
  Il faut donc rebaser cette branche après la fusion de p1-5c, puis exiger
  une CI verte.
- Matériel physique : non validé.

## Problèmes connus

- Mono-cœur : pile syscall partagée, pas de SMP.
- Un seul processus utilisateur à la fois ; pas de table de processus persistante.
- VFS en lecture seule (SYSFS) ; transport virtio moderne non pris en charge.
- Les écritures/lectures utilisateur des syscalls valident les plages, mais une
  faute Ring 0 reste fatale par conception.
- Conflit d'agents : un autre agent travaille dans la copie de travail partagée
  (branches `p1-5c-ls-readdir`, `p2-1-stat-spawn`). Cette branche part de
  `origin/main` et isole tout son travail ; la branche `p1-4-user-fault-recovery`
  poussée plus tôt est obsolète car empilée sur `p1-5c`.

## Dépendances

- PR p1-5c (SYS_READDIR / ls) en attente de fusion : elle touche aussi
  `kernel/src/syscall.rs`, `main.rs`, `interrupts.rs` et la CI. Prévoir une
  résolution de conflit mineure lors de la fusion de cette branche.

## Prochaine action exacte

1. Pousser `p1-5-user-fault-recovery` et ouvrir une PR vers `main`.
2. Attendre tous les contrôles sur le SHA exact de la PR ; corriger si rouge.
3. Une fois p1-5c fusionnée, rebaser cette branche sur le nouveau `main`,
   résoudre les conflits signalés, relancer tests + QEMU, puis fusionner.
4. Ensuite : table de processus durable, terminaison et IPC (phase B/C).
