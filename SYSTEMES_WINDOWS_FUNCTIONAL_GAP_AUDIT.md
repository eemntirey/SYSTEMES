# SYSTEMES — Audit fonctionnel comparatif avec Windows 11

> Audit documentaire et code de référence réalisé le 25 novembre 2026 sur la branche `main`.
> Périmètre : OS autonome Rust `no_std`, UEFI x86-64, QEMU/OVMF. Cet audit n'assimile pas une fonction présente dans une PR ouverte à une livraison.
> **Précaution de preuve :** les statuts sont conservateurs. `IMPLEMENTED` signifie que le code est dans `main` et qu'une validation pertinente est décrite par le workflow CI ou par une preuve de fusion identifiée. Lorsqu'une preuve d'exécution manque pour le périmètre précis, le statut reste `PARTIAL` ou `NOT_VERIFIED`.

## 0. Inventaire des trois couches annoncées

L'inventaire du contenu suivi dans `main` ne confirme pas les trois couches annoncées dans le cahier des charges :

| Couche | Constat dans `main` | Statut d'inventaire | Règle de comptabilisation |
|---|---|---|---|
| Noyau Rust `no_std` | `boot/`, `kernel/`, cible UEFI x86-64, code et workflows QEMU/OVMF présents | `PRESENT` | Seules les capacités du noyau réellement compilées et testées sont créditées à l'OS bootable |
| Moteur natif .NET | Aucun projet `.sln` / `.csproj` ni moteur .NET opérationnel identifié dans le contenu audité | `NOT_IDENTIFIED_IN_MAIN` | Ne pas présenter un moteur hôte historique ou externe comme un service du noyau |
| UI React | Aucun projet React / `package.json` / point d'entrée React identifié dans le contenu audité | `NOT_IDENTIFIED_IN_MAIN` | Ne pas créditer une interface web ou Windows-hosted comme environnement graphique natif |

Le `README.md` racine décrit l'architecture actuelle comme chargeur UEFI, noyau Rust, userland, image EFI/ISO et documentation. Il précise aussi que les anciens composants d'interface et d'intégration Windows présents dans l'historique ne constituent pas une édition du produit. En conséquence, **les couches .NET et React ne sont pas démontrées dans `main`** et ne participent pas à l'évaluation fonctionnelle du noyau.

Si ces deux couches existent dans des dépôts séparés ou dans une distribution externe, elles devront faire l'objet d'un inventaire distinct avec leurs dépôts, SHA et preuves propres ; elles ne seront pas fusionnées avec les capacités de l'OS bootable.

## 1. Légende des statuts

| Statut | Signification |
|---|---|
| `IMPLEMENTED` | Code présent dans `main`, compilé, testé et preuve QEMU reproductible disponible pour le périmètre décrit. |
| `PARTIAL` | Une partie fonctionne, mais le périmètre Windows comparable n'est pas couvert. |
| `MISSING` | Aucun code fonctionnel correspondant n'a été identifié dans `main`. |
| `BROKEN` | Code présent, mais régression ou échec reproductible identifié. |
| `NOT_VERIFIED` | Code ou capacité revendiquée, mais preuve d'exécution suffisante non établie. |

Une PR ouverte n'est jamais une preuve de livraison. Un test hôte seul ne prouve pas le fonctionnement matériel ou QEMU. Une réussite QEMU ne prouve pas la compatibilité avec un PC physique.

## 2. Résumé exécutif

SYSTEMES est aujourd'hui un noyau autonome UEFI x86-64 avec mémoire protégée, exécution Ring 3, ordonnanceur, démarrage SMP expérimental, syscalls minimales, shell UART et stockage virtio-blk/SYSFS en lecture seule. Il ne constitue pas encore un OS de bureau comparable à Windows 11.

Les blocages les plus importants pour un usage quotidien sont :
1. système de fichiers inscriptible et persistance durable ;
2. ABI utilisateur et environnement de processus complets ;
3. pilotes d'entrée, stockage physique et périphériques ;
4. réseau ;
5. environnement graphique ;
6. écosystème d'applications, comptes, permissions et installation.

### Vue synthétique

| Domaine fonctionnel | Statut global | Écart avec Windows 11 |
|---|---|---|
| Démarrage et noyau | `PARTIAL` | Bring-up UEFI démontré, compatibilité matérielle limitée |
| Mémoire et protection | `IMPLEMENTED` / `PARTIAL` | Socle noyau avancé, fonctionnalités OS supérieures incomplètes |
| Processus et ordonnancement | `PARTIAL` | Cycle de vie Ring 3 livré, API processus et threads incomplètes |
| ABI et bibliothèques | `PARTIAL` | 9 syscalls, pas de libc ni runtime utilisateur complet |
| Stockage et fichiers | `PARTIAL` | Lecture seule, pas de persistance utilisateur |
| Pilotes matériels | `MISSING` / `PARTIAL` | Pas de PC de bureau utilisable |
| Réseau | `MISSING` | Pas de pile réseau opérationnelle démontrée |
| Interface graphique | `MISSING` | Pas de bureau natif |
| Applications | `MISSING` | Shell minimal uniquement |
| Sécurité utilisateur | `PARTIAL` | Isolation mémoire Ring 3, mais pas de comptes/ACL |
| Installation et maintenance | `PARTIAL` | Artefacts UEFI/QEMU, pas d'installateur complet |

## 3. Matrice détaillée des écarts

### 3.1 Démarrage, noyau et gestion système

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| B01 | Démarrage UEFI x64 | Chargeur UEFI et entrée noyau présents ; workflow construit une image et démarre QEMU/OVMF | `IMPLEMENTED` | Ajouter validation sur plusieurs firmwares et PC physiques |
| B02 | Noyau protégé | Noyau Rust `no_std`, cible `x86_64-unknown-none` | `IMPLEMENTED` | Périmètre noyau autonome, pas compatibilité Windows |
| B03 | Gestion des exceptions CPU | IDT, stubs, diagnostics ; #BP reprenable | `IMPLEMENTED` | Les exceptions fatales ne sont pas toutes récupérables |
| B04 | GDT, TSS, piles IST | Descripteurs, TSS et IST vérifiés en CI/QEMU | `IMPLEMENTED` | Essais matériels NMI/MCE réels non fournis |
| B05 | Gestion mémoire physique | Allocateur de frames et tests présents | `IMPLEMENTED` | Pression mémoire longue durée et fragmentation à mesurer |
| B06 | Mémoire virtuelle | Tables x86-64, espaces privés et changement CR3 | `IMPLEMENTED` | Gestion avancée mémoire et partage mémoire à compléter |
| B07 | NX, W^X, CR0.WP | Protections et scénarios négatifs QEMU dans le workflow | `IMPLEMENTED` | Conserver les tests négatifs en non-régression |
| B08 | SMEP / SMAP | Activation conditionnelle CPUID, accès user encadrés, scénarios QEMU | `IMPLEMENTED` | Dépend du support CPU |
| B09 | SMP : démarrage des AP | Trampoline INIT/SIPI ; CI QEMU/OVMF `-smp 4` vérifie les 3 AP, compteurs privés, compteur partagé et SpinLock cross-core | `IMPLEMENTED` | Preuve QEMU, pas validation sur matériel physique ; ne pas confondre bring-up avec migration des tâches utilisateur |
| B10 | Tables CPU-locales et timers APIC | GDT/IDT/TSS et configuration SYSCALL par CPU ; timers Local APIC AP ; jobs QEMU vérifient les schedulers per-CPU et la migration sur 2/4 CPU | `IMPLEMENTED` | Périmètre QEMU validé ; compatibilité matérielle réelle et stress prolongé restent à prouver |
| B11 | Ordonnanceur multicœur | Runqueues CPU et chemins de migration présents | `PARTIAL` | Migration fiable, absence de double exécution et stress soutenu à prouver |
| B12 | Shootdown TLB distant | PR #103 fusionnée ; CI QEMU vérifie les AP en ligne et le marqueur IPI/ACK distant sur 2/4 CPU | `PARTIAL` | Les tâches Ring 3 restent CPU-affines au BSP : aucune traduction d'un espace utilisateur effectivement actif sur un AP n'est encore invalidée dans un test ; étendre la preuve avant de déclarer le shootdown multi-espace complet |
| B13 | Horloge monotone / timer | Timer Local APIC calibré et tests QEMU | `IMPLEMENTED` | Horloge murale/RTC et gestion énergie restent distinctes |
| B14 | Gestion énergie / veille / reprise | Aucun cycle complet ACPI S-state ou reprise système validé identifié | `MISSING` | ACPI power management, suspend/resume, arrêt propre |

### 3.2 Processus, threads et ABI utilisateur

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| P01 | Exécution en Ring 3 | Transition CPL3, espace utilisateur et ELF de test intégré | `IMPLEMENTED` | Chargement d'exécutables depuis disque manquant |
| P02 | Création et terminaison de tâches Ring 3 | Propriétaire Process, scheduler, terminaison non-retournante et reaper livrés ; PR #101 fusionnée | `IMPLEMENTED` | Ne signifie pas API processus POSIX complète |
| P03 | Nettoyage et réclamation mémoire SMP | Pins, masque CPU, reaper différé et protocole TLB documentés | `PARTIAL` | La preuve distante AP de PR #103 doit être fusionnée puis validée sur son SHA |
| P04 | Processus multiples persistants | Plusieurs tâches Ring 3 de test, tables de fichiers par processus | `PARTIAL` | Gestion de sessions, lancement depuis disque, parent/enfant et cycle de vie utilisateur à finaliser |
| P05 | `fork` | Aucun syscall `fork` dans le dispatcher 0–8 | `MISSING` | Implémenter sémantique de duplication d'espace et héritage des handles |
| P06 | `exec` | ELF intégré ; pas de syscall exec ni chargement général depuis VFS | `PARTIAL` | Chargeur ELF depuis fichier, remplacement atomique de l'image |
| P07 | `waitpid` | Aucun syscall correspondant | `MISSING` | Attente, codes de sortie, zombies et réclamation |
| P08 | Threads utilisateur / `clone` | Pas d'API utilisateur threads complète identifiée | `MISSING` | Création, synchronisation, TLS, piles et terminaison |
| P09 | Préemption noyau | Ordonnanceur préemptif et tests timer QEMU | `IMPLEMENTED` | Mesurer équité, latence et charge SMP |
| P10 | Contexte FPU/SIMD | Sauvegarde/restauration FXSAVE64/FXRSTOR64 testée | `IMPLEMENTED` | XSAVE/AVX et états étendus à traiter |
| P11 | Appels système | Syscalls 0 à 8 : version, read/write, exit, yield, uptime, open, close, readdir | `PARTIAL` | ABI minimale, pas POSIX |
| P12 | Mémoire utilisateur `mmap/munmap/brk` | Aucun dispatch identifié | `MISSING` | Allocateur virtuel utilisateur et validation des plages |
| P13 | IPC : pipe, dup, ioctl | Aucun dispatch identifié | `MISSING` | Descripteurs, objets noyau et synchronisation |
| P14 | Temps : `clock_gettime/nanosleep` | Uptime et sommeil des tâches noyau existent ; ABI utilisateur incomplète | `PARTIAL` | Horloge monotone utilisateur, échéances et errno |
| P15 | Répertoire courant : `chdir/getcwd` | Chemins absolus seulement dans l'ABI actuelle | `MISSING` | État cwd par processus et résolution relative |
| P16 | Gestion des erreurs | Erreurs négatives documentées pour les syscalls existants | `PARTIAL` | Compléter errno et contrats pour les nouveaux appels |

### 3.3 Stockage, systèmes de fichiers et périphériques de bloc

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| S01 | Pilote de disque | Virtio-blk PCI legacy/transitional, lecture DMA synchrone | `PARTIAL` | Pas de NVMe/AHCI ni transport virtio moderne validé |
| S02 | VFS | Montage unique SYSFS, opérations open/read/close/readdir | `PARTIAL` | VFS explicitement read-only |
| S03 | Création de fichiers | Non exposée par le VFS de main | `MISSING` | create/write et gestion des métadonnées |
| S04 | Écriture de fichiers | Pas de chemin VFS persistant | `MISSING` | write, truncate, append, permissions |
| S05 | Répertoires modifiables | mkdir/unlink/rename absents du VFS actuel | `MISSING` | Opérations atomiques et cohérence |
| S06 | FAT32 lecture/écriture | Aucun pilote FAT32 RW livré identifié | `MISSING` | Lecture/écriture, allocation clusters, FSInfo et vérification |
| S07 | Cache bloc write-back | Pas de cache write-back fusionné identifié dans main | `MISSING` | Cache, éviction, writeback et erreurs |
| S08 | Flush / fsync / barrières | Pas de flush durable validé | `MISSING` | VIRTIO_BLK_F_FLUSH ou équivalent |
| S09 | Persistance après reboot | SYSFS chargé en mémoire depuis le disque ; écriture/relecture persistante non démontrée | `MISSING` | QEMU write → poweroff → reboot → reread + vérification FS |
| S10 | tmpfs | Primitive proposée dans PR #83 mais PR ouverte | `MISSING` | Ne pas compter la PR ouverte |
| S11 | devfs (/dev/null, zero, console) | Primitive proposée dans PR #83 mais PR ouverte | `MISSING` | Intégration VFS et tests d'usage requis |
| S12 | Montage multiple / démontage | VFS actuel à montage unique | `MISSING` | mount/umount, références et démontage sûr |
| S13 | Vérification/récupération FS | Aucun fsck-like livré identifié | `MISSING` | Vérification structurelle et récupération |

### 3.4 Pilotes et matériel PC

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| D01 | PCI/PCIe générique | Détection PCI limitée au périphérique virtio-blk ciblé | `PARTIAL` | Énumération bus, BAR, capabilities, MSI/MSI-X |
| D02 | NVMe | Aucun pilote opérationnel identifié | `MISSING` | Queue admin/I/O, namespaces, IRQ et tests |
| D03 | AHCI/SATA | Aucun pilote opérationnel identifié | `MISSING` | Contrôleur, DMA et commandes ATA |
| D04 | USB xHCI | Aucun pilote identifié | `MISSING` | Énumération, control/bulk/interrupt transfers |
| D05 | Clavier PS/2 | Pas de pilote complet IRQ1/8042 identifié | `MISSING` | Entrée réelle et file d'événements |
| D06 | USB HID clavier/souris | Aucun pilote identifié | `MISSING` | HID boot/report protocol |
| D07 | Souris / pointeur | Aucun chemin d'entrée utilisateur complet identifié | `MISSING` | Événements et intégration graphique |
| D08 | Framebuffer GOP | Adresse/contrat framebuffer transmise par UEFI ; affichage natif utilisable non démontré | `NOT_VERIFIED` | Pilote framebuffer, rendu texte et tests visuels |
| D09 | GPU accéléré | Aucun pilote GPU opérationnel identifié | `MISSING` | Dépendance forte au matériel et à une API graphique |
| D10 | Audio HDA | Aucun pilote identifié | `MISSING` | Enumeration PCI, codec, DMA, sorties |
| D11 | RTC / horloge murale | Horloge monotone présente ; horloge murale fiable non démontrée | `NOT_VERIFIED` | RTC/UEFI time et fuseaux |
| D12 | Compatibilité PC physique | Les preuves disponibles sont QEMU/OVMF ; pas de matrice matérielle réelle | `NOT_VERIFIED` | Tester plusieurs cartes mères, CPU et contrôleurs |

### 3.5 Réseau et connectivité

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| N01 | Ethernet | Aucun pilote réseau opérationnel identifié | `MISSING` | Commencer par virtio-net QEMU |
| N02 | ARP / IPv4 / IPv6 | Aucune pile fonctionnelle identifiée | `MISSING` | Couche liaison et réseau |
| N03 | TCP / UDP / sockets | Aucun transport/socket utilisateur identifié | `MISSING` | Pile transport et ABI sockets |
| N04 | DNS / DHCP | Aucun client/service identifié | `MISSING` | Configuration réseau automatique |
| N05 | Wi-Fi | Aucun pilote ni supplicant | `MISSING` | Report explicite hors première version |
| N06 | Bluetooth | Aucun contrôleur/protocole | `MISSING` | Report explicite hors première version |

### 3.6 Interface graphique et expérience utilisateur

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| G01 | Serveur graphique/compositeur | Aucun serveur graphique natif identifié | `MISSING` | Framebuffer, surfaces, composition |
| G02 | Gestionnaire de fenêtres | Absent | `MISSING` | Création, déplacement, focus et fermeture |
| G03 | Bureau / barre des tâches | Absent du noyau autonome | `MISSING` | Shell graphique natif |
| G04 | Menu principal / lanceur | Absent | `MISSING` | Catalogue et lancement d'applications |
| G05 | Gestionnaire de fichiers graphique | Absent | `MISSING` | Dépend d'un VFS inscriptible |
| G06 | Terminal graphique | Shell UART minimal uniquement | `PARTIAL` | Terminal graphique, TTY et PTY |
| G07 | Paramètres système | Absent | `MISSING` | Services système et configuration persistante |
| G08 | Accessibilité / DPI / multi-écrans | Aucun support identifié | `MISSING` | Entrées, rendu et configuration d'affichage |

### 3.7 Écosystème logiciel, sécurité et administration

| ID | Fonction comparable Windows 11 | État SYSTEMES dans main | Statut | Écart / preuve attendue |
|---|---|---|---|---|
| E01 | Bibliothèque standard C | Absente | `MISSING` | Sous-ensemble libc/POSIX et ABI stable |
| E02 | Runtime Rust utilisateur | Pas de runtime std-like identifié | `MISSING` | Support allocateur, syscalls et panic |
| E03 | Chargeur de programmes depuis disque | ELF de test intégré en mémoire | `PARTIAL` | Chargement ELF depuis VFS |
| E04 | Gestionnaire de paquets | Absent | `MISSING` | Format, dépôt, signature et transaction |
| E05 | Catalogue d'applications | Absent | `MISSING` | Métadonnées et lancement |
| E06 | Comptes utilisateurs | Absent | `MISSING` | Identités, sessions et authentification |
| E07 | Permissions POSIX/ACL | Isolation Ring 3 et pages U/S ; permissions fichiers absentes | `PARTIAL` | UID/GID, mode bits, ACL et contrôle d'accès |
| E08 | Sandbox applicative | Isolation d'adressage Ring 3 présente | `PARTIAL` | Capacités, restrictions syscall, objets et politiques |
| E09 | Gestionnaire de services | Absent | `MISSING` | Supervision, dépendances, redémarrage |
| E10 | Journaux système | Console série/debug seulement | `PARTIAL` | Logs structurés, rotation, persistance |
| E11 | Mise à jour système | Absente | `MISSING` | Mise à jour atomique, rollback et signature |
| E12 | Récupération système | Absente | `MISSING` | Mode recovery, diagnostic et restauration |
| E13 | Installateur EFI | Workflow CI fabrique une image de test ; installateur disque utilisateur absent | `PARTIAL` | Partitionnement, confirmation et installation sûre |
| E14 | Stabilité 72 heures | Aucun test d'endurance 72 h documenté | `NOT_VERIFIED` | Stress continu, mémoire, stockage et réseau |

## 4. PR ouvertes à ne pas compter comme fonctionnalités livrées

| PR | Sujet | Traitement dans cet audit |
|---|---|---|
| [#103](https://github.com/eemntirey/SYSTEMES/pull/103) | Preuve du shootdown TLB distant avec AP réellement en ligne | En attente. Le contenu de la PR décrit des preuves QEMU 1/2/4 CPU, mais ces résultats ne sont pas attribuables à `main` avant fusion et CI verte sur le SHA final. |
| [#83](https://github.com/eemntirey/SYSTEMES/pull/83) | Fondations écriture virtio, cache, tmpfs et devfs | En attente. La PR précise elle-même que le VFS reste RO, que FAT32 RW et flush durable manquent, et que la persistance après reboot n'est pas démontrée. |

La PR #101 est documentée comme fusionnée dans `docs/STATE.md`, avec 14 contrôles obligatoires verts sur son SHA validé. Elle constitue la preuve de référence pour le cycle de vie Ring 3 et la réclamation différée, sans valider automatiquement les fonctionnalités ultérieures.

## 5. Incohérences documentaires à corriger avant WP1.1

| Document | Constat | Correction requise |
|---|---|---|
| `kernel/README.md` | Présente P1-6c comme ordonnanceur SMP avancé, mais sa section limites parle encore de pile SYSCALL BSP statique et de noyau mono-cœur | Réécrire les capacités à partir du code compilé et distinguer SMP noyau, Ring 3 et migration |
| `AUTONOMOUS_DEVELOPMENT.md` | Présente Phase B/P1-6a comme état courant et P1-6b comme prochaine étape ; journal historique antérieur à la séquence P0 récente | Remplacer l'état courant par un résumé daté et garder l'historique dans une section explicitement historique |
| `docs/SYSCALL_ABI.md` | Décrit correctement les syscalls 0–8, mais le plan d'extension n'est pas une ABI livrée | Garder 0–8 comme ABI réelle ; ajouter une section « non implémenté / réservé » sans attribuer de numéros non décidés |
| `docs/STATE.md` | Décrit les fusions P0 et les preuves PR #101 ; certaines sections sont explicitement historiques | Préserver les preuves historiques, ajouter un pointeur vers cet audit et actualiser l'état SMP après traitement #103 |
| `docs/PLAN_PROCHAINE_ETAPE.md` | Donne une roadmap P1/P2/P3 plus récente que l'ancien journal autonome | Faire de ce document la référence de séquencement ou synchroniser les autres documents avec lui |

## 6. Ordre de comblement recommandé

L'ordre respecte les dépendances techniques. Pas de bureau graphique avant stockage, entrées et réseau suffisamment stables.

| Ordre | Work package | Résultat vérifiable attendu | Dépend de |
|---|---|---|---|
| 0 | Audit + réconciliation documentaire | Matrice et documents alignés sur `main` | — |
| 1.1 | Preuve SMP / TLB distant | AP en ligne prouvés, compteur de travail par AP, ACK shootdown 2/4 CPU | PR #103 |
| 1.2 | Migration de tâches | Migration N fois, contexte/stack/CR3/TLB intègres, absence de double exécution | 1.1 |
| 1.3 | Processus et threads | clone/threads, création, attente, terminaison et reaping | 1.1–1.2 |
| 1.4 | ABI utilisateur | syscalls documentées et testées, errno et compatibilité versionnée | 1.3 |
| 2.1 | VFS inscriptible | create/write/mkdir/unlink/rename/truncate/stat via VFS | 1.4 |
| 2.2 | FAT32 RW | création, écriture, reboot et relecture | 2.1 |
| 2.3 | Durabilité | flush/fsync, barrières et test d'arrêt brutal | 2.2 |
| 2.4 | tmpfs/devfs | montages intégrés et tests utilisateur | 2.1 |
| 3 | PCI, stockage, USB, HID, framebuffer, audio | Pilotes incrémentaux avec tests QEMU et matériel | 2 |
| 4 | Ethernet et pile réseau | DHCP/DNS/TCP/UDP/sockets testés | 3 |
| 5 | Graphique natif | compositeur, fenêtres, bureau, entrée utilisateur | 2–4 |
| 6 | Écosystème logiciel | libc, runtime Rust, paquets et applications | 1–5 |
| 7 | Sécurité/administration | comptes, permissions, sandbox, services, récupération | 2–6 |
| 8 | Distribution | ISO installable, partitionnement protégé, matériel, endurance | 2–7 |

## 7. Conditions de clôture

Une ligne ne passe à `IMPLEMENTED` que lorsque les éléments suivants sont rattachés au même changement livré :
- code présent dans `main` ;
- compilation de production et tests hôte verts ;
- preuve QEMU/OVMF reproductible avec script et marqueurs, ou essai matériel documenté ;
- non-régression des 14 contrôles obligatoires de référence de la PR #101 et des tests SMP/TLB/Ring 3 ;
- documentation mise à jour ;
- SHA exact et liens CI conservés dans cette matrice.

Ne jamais déduire un résultat QEMU d'un test unitaire. Ne jamais déduire un résultat matériel d'un démarrage QEMU. Ne jamais déclarer un jalon complet sur la base d'une PR ouverte.

## 8. Sources inspectées sur main

- `kernel/README.md` — blob `4caea53a15d403c8f1bc5152c19694bad319b7c9`
- `AUTONOMOUS_DEVELOPMENT.md` — blob `fb56499934e81e810381abffffb187784f856ba9`
- `docs/SYSCALL_ABI.md` — blob `7cc8602d236d4c75ab06b3298e4d1205f9ecfa57`
- `kernel/Cargo.toml` — blob `63bbdac2298fa0fe87d60a798d4940e5db1ab0ef`
- `.github/workflows/kernel.yml` — blob `41c08258096f821091a8fff4da40fba573bf17ec`
- `kernel/src/smp.rs` — blob `da3a34a3b1d38284353b773ee2bf3da4891b3df5`
- `kernel/src/scheduler.rs` — blob `f7a5b48598327bd2b29ff3414e7bdcf1a743e3cf`
- `kernel/src/process.rs` — blob `7408f339c1dd1c1054072db5cc8528fc513364f7`
- `kernel/src/syscall.rs` — blob `7b00834f2c7e8d5c36b30dd41602ca57e0936357`
- `kernel/src/vfs.rs` — blob `88008d40fb0249f91490a4a26d3633a03ccf748c`
- `kernel/src/simple_fs.rs` — blob `4128cfda2722cd612bf0ee5f27d4137d4724a30c`
- `kernel/src/virtio_blk.rs` — blob `bbe0fe9a754c0a09b469cb1903dd63b0600576bd`
- `docs/STATE.md` — blob `8f17814a47e3ee7829eb548e2b0dfb199528e503`
- `docs/PLAN_PROCHAINE_ETAPE.md` — blob `499dcabcf880f022627aa561b03dd5469fd20530`

Les SHA ci-dessus identifient les blobs consultés ; ils ne sont pas le SHA du commit de branche. Le commit exact de l'audit et son run CI doivent être ajoutés lors de la création de la PR.
