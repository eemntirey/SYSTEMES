# État du noyau — suivi P0

**Actualisation :** 2026-11-14  
**Branche de référence :** `main`  
**Commit de référence :** `d3e7c090a585b0b9dbf5c6dfef17519a38948d82` (squash de la PR #88)

Ce document conserve l'historique de l'intégration #86 et du durcissement #88. L'état le plus récent de la récupération synchrone Ring 3 est décrit en section 8 ; la section 6 est un historique de la feuille de route antérieure.

## 1. État global P0

**Étape 1 — isolation Ring 3 / SYSCALL par CPU : intégrée et validée par la CI de la PR #86.**  
**P0 complet : non déclaré terminé.** L'audit de durcissement décrit en section 6 reste à réaliser avant toute correction supplémentaire.

La PR [#86](https://github.com/eemntirey/SYSTEMES/pull/86) a été fusionnée le 14 novembre 2026. Son commit source est `0ff021dc42cc3593cbcddf382a25aaf6e3704450`, et son commit squash est `484c01e82568994bd9244e519839367f1ddc09cd`.

## 2. Acquis de la PR #86

### 2.1 Contexte Ring 3 et SYSCALL CPU-local

Dans `kernel/src/interrupts.rs` :

- `SyscallCpuLocal` contient la pile d'entrée SYSCALL, le RSP utilisateur sauvegardé, le RSP noyau sauvegardé, les drapeaux `ring3_active` et `exit_validated`, le RSP utilisateur attendu et le statut de sortie.
- `CpuContext` regroupe ce contexte SYSCALL avec la GDT, la TSS, l'IDT et les piles d'interruption.
- `CPU_CONTEXTS` est indexé par slot CPU. Le commentaire du type indique que `GS_BASE` pointe sur le contexte du CPU courant en Ring 0.
- Le code de la PR traite `SWAPGS` sur SYSCALL, INT 0x80, exceptions, IRQ et interruption APIC spurious. Le chemin d'exception tient compte des fenêtres de transition NMI/MCE.

Le modèle documenté par la PR est : en Ring 0, `GS_BASE` désigne le contexte CPU-local ; en Ring 3, le GS utilisateur est nul. Les états de sortie précédemment globaux ont été déplacés dans `SyscallCpuLocal`.

### 2.2 Initialisation des AP

Dans `kernel/src/smp.rs`, l'entrée Rust de chaque AP initialise son contexte GDT/IDT/TSS, appelle `interrupts::initialize_syscalls()`, puis vérifie la configuration SYSCALL pour son slot CPU. En cas d'échec, l'AP est immobilisé au lieu de poursuivre avec une configuration incomplète.

La pile de démarrage AP est allouée et publiée par APIC ID avant l'envoi des INIT/SIPI. La page trampoline basse mémoire est réservée par `reserve_trampoline_page()` et conservée pour le démarrage des AP.

### 2.3 Rendez-vous Ring 3 SMP

La feature `qemu-ring3-smp-test` prépare un espace utilisateur partagé, des piles distinctes et une barrière de départ. Les AP publient leur état prêt puis attendent le rendez-vous ; le BSP libère le test après le démarrage des AP.

Le scénario exécute simultanément les transitions Ring 3/SYSCALL sur les CPU participants et vérifie l'indépendance des champs CPU-locaux. Les chemins de sortie sont alternés entre `SYS_EXIT` et `INT 0x80` avec des statuts distincts. Le scénario vérifie également des interruptions timer APIC provenant de Ring 3 sur les CPU dont les interruptions sont activées.

Dans `kernel/src/main.rs`, l'espace utilisateur du test n'est préparé qu'après le rendez-vous de démarrage des AP, afin que les allocations de tables de pages ne consomment pas la page trampoline réservée.

### 2.4 Dépendance de feature nécessaire

Dans `kernel/Cargo.toml` :

```toml
qemu-smp-test = []
qemu-ring3-smp-test = ["qemu-smp-test"]
```

Cette dépendance est intentionnelle. Le test Ring 3 SMP a besoin du chemin de démarrage des processeurs secondaires et de la réservation de la page trampoline sous 1 Mio. Sans l'activation de `qemu-smp-test`, le scénario Ring 3 peut démarrer sans les fondations de bring-up AP requises.

## 3. Matrice QEMU 1 / 2 / 4 CPU

Script : [`scripts/qemu-concurrency-matrix.sh`](https://github.com/eemntirey/SYSTEMES/blob/main/scripts/qemu-concurrency-matrix.sh).

- La liste par défaut est `CPUS="1 2 4"` ; les valeurs acceptées sont strictement 1, 2 et 4.
- Les cas sont exécutés séquentiellement dans un seul job, chaque cas supprimant l'ancien journal avant le démarrage QEMU et sauvegardant son propre instantané.
- Pour chaque configuration, le script exécute `qemu-ring3-smp-test`. Pour 1 CPU, il exécute aussi le démarrage de référence `single-core-smoke`. Pour 2 et 4 CPU, il exécute en plus `qemu-smp-test` et `qemu-smp-scheduler-test`.
- Chaque cas échoue si QEMU retourne un code non nul, si le journal est absent, si `SYSTEMES_KERNEL_FAIL:` apparaît ou si `SYSTEMES_KERNEL_OK` manque.
- Le cas Ring 3 vérifie le nombre d'AP attendu (`CPU - 1`), le rendez-vous MADT/AP, la preuve d'exécution SYSCALL simultanée, et la preuve d'interruptions timer APIC depuis Ring 3.
- Le script imprime un verdict `case=... smp=... verdict=PASS|FAIL` pour chaque cas, puis `MATRIX_OK` uniquement si aucun cas n'a échoué.

La matrice est séquentielle : elle fournit un verdict par cas QEMU, pas un verdict indépendant pour chaque CPU au sein d'un cas. Les preuves par CPU sont portées par les marqueurs et diagnostics série émis par le noyau.

## 4. Validation CI de la PR #86

Workflow run [#43593078770](https://github.com/eemntirey/SYSTEMES/actions/runs/43593078770), associé au SHA source final `0ff021dc42cc3593cbcddf382a25af6e3704450` : **13 jobs sur 13 terminés avec succès**.

Jobs observés :

1. SMP application processor bring-up
2. QEMU hardware fault — SMEP must block supervisor execution of a user page
3. SMP bring-up + BSP scheduler regression
4. QEMU hardware fault — NX execution must fault
5. Production boot — userland shell with SYSFS disk
6. QEMU hardware fault — SMAP must block supervisor read of a user page
7. no_std contract tests and x86_64 target check
8. QEMU hardware fault — RX write must fault
9. QEMU SMP concurrency matrix (1, 2 and 4 CPUs)
10. Production boot — userland shell without disk
11. QEMU hardware fault — NMI IST2 stack switch
12. QEMU hardware fault — Machine Check IST3 stack switch
13. Build UEFI loader and boot kernel in QEMU/OVMF

La réussite de cette CI valide les scénarios effectivement exécutés par ces jobs sur le SHA indiqué. Elle ne constitue pas une preuve de couverture des cas supplémentaires listés dans la phase de durcissement ci-dessous.

## 5. Périmètre explicitement non modifié

La PR #86 n'a pas modifié le code de l'ordonnanceur. La matrice continue de lancer ses tests de régression SMP/scheduler, mais l'objectif de cette étape était l'isolation des transitions Ring 3/SYSCALL.

Les fonctionnalités de processus, PID, tables de fichiers par processus, `fork`, VFS et ELF n'ont pas été livrées par la PR #86 et ne doivent pas être considérées comme acquises sur la seule base de cette fusion.

## 6. Travail restant — durcissement P0

La phase suivante est un audit du chemin d'entrée/sortie Ring 3 et SYSCALL sur le code courant, avant toute modification. Elle doit classer chaque point BUG CONFIRMÉ / RISQUE / OK et ne retenir que les deux premières catégories lorsqu'elles sont justifiées par le code.

Points à vérifier :

1. Programmation et vérification de STAR, LSTAR, SFMASK et EFER.SCE sur le BSP et chaque AP.
2. Pile d'entrée SYSCALL, pile noyau courante par thread, RSP0 de la TSS, IST et chargement TR sur chaque CPU.
3. Appariement SWAPGS sur SYSCALL, INT 0x80, IRQ, exceptions, NMI et #MC, y compris les fenêtres de transition et les chemins d'erreur.
4. Choix SYSRET/IRETQ, canonicalité des adresses de retour, restauration des registres et nettoyage de l'état sensible.
5. Absence d'état mutable partagé entre CPU sur les chemins de transition et syscall.
6. Validation des pointeurs utilisateur, bornes/canonicalité, et discipline SMAP STAC/CLAC autour des accès.
7. Fenêtres d'interruption entre l'entrée SYSCALL et la mise en place de la pile noyau ; cohérence de SFMASK.
8. Traitement des fautes Ring 3 (#PF/#GP) sans corruption de l'état d'un autre CPU.

Aucun bug n'est présumé dans cette liste : chaque constat doit être établi à partir du code actuel et, pour être corrigé, reproduit par un test QEMU qui échoue avant la correction. Les cas de test à ajouter si l'audit les justifie comprennent les syscalls concurrents sur 1/2/4 CPU, les pointeurs invalides/non canoniques, les transitions répétées sous timer/IPI et une faute Ring 3 simultanée à des syscalls sur un autre CPU.

### Critères de clôture

- Les correctifs doivent rester limités aux fichiers nécessaires ; aucun changement de scheduler sans nécessité démontrée et signalée.
- Les tests doivent prouver le défaut avant correction et le comportement corrigé après.
- Le build de production sans feature de test doit rester valide.
- Tous les jobs CI doivent être verts sur le SHA exact de la PR.
- Les résultats de la matrice doivent inclure le verdict de chaque configuration, le nombre d'AP démarrés et l'absence de marqueur d'erreur noyau.
- Aucun merge avant CI entièrement verte sur le SHA final et accord explicite.

## 7. Historique de l'actualisation

L'audit initial du 13 novembre décrivait notamment la pile SYSCALL et plusieurs états Ring 3 comme globaux, car il portait sur une base antérieure à la PR #86. Ces constats restent utiles comme historique, mais ne doivent plus être présentés comme l'état du code après le commit squash ci-dessus lorsque les champs concernés ont été déplacés dans `SyscallCpuLocal`.


## 8. P0 — récupération des fautes Ring 3 synchrones (PR #89)

**Périmètre : niveau 1 uniquement.** La branche p0-ring3-fault-handling ajoute le retour de #PF/#GP CPL3 vers la continuation noyau enregistrée par systemes_enter_ring3. Le CS sauvegardé par le processeur est la source de vérité pour le CPL ; les fautes CPL0 restent fatales, y compris pendant un syscall. Les informations de faute sont conservées dans SyscallCpuLocal (vecteur, code d'erreur, CR2 et RIP).

Le nettoyage du test d'espace utilisateur partagé est différé jusqu'au retour de tous les participants et à leur restauration du CR3 noyau. Les frames sont ensuite libérées une seule fois et le compteur de frames libres est comparé à sa valeur initiale. Les IST existantes restent inchangées : #DF=IST1, NMI=IST2, #MC=IST3.

Les scénarios QEMU #PF, #GP, fautes répétées et pointeurs syscall invalides sont ajoutés à scripts/qemu-concurrency-matrix.sh pour 1/2/4 CPU. **Leur résultat n'est pas déclaré validé tant que la CI n'a pas terminé avec succès sur le SHA final.**

Le niveau 2, terminaison d'une tâche Ring 3 ordonnançable et sélection d'une autre tâche, est explicitement hors périmètre. Sa conception est décrite dans [P0_RING3_TASK_TERMINATION_DESIGN.md](P0_RING3_TASK_TERMINATION_DESIGN.md). Aucun fichier scheduler ne doit être modifié dans cette PR.


Le test d'une faute CPL0 n'est pas intégré à la matrice 1/2/4 CPU : le chemin fatal `cli; hlt` immobiliserait le CPU de test avant le rendez-vous SMP et `--debug-exit`. Une validation dédiée demanderait un boot QEMU isolé et un point de terminaison réservé au test ; le chemin de production reste fatal.


## 9. P0 — contrat scheduler de terminaison Ring 3 (incrément 1, PR en cours)

La branche `p0-ring3-task-termination` ajoute le contrat scheduler 4d sans raccorder encore l'entrée Ring 3, le CR3, le TSS/GS, le trampoline ni le reaper.

- `TaskExitReason` est un enum `Copy` à payload fixe (sortie volontaire, #PF, #GP), sans allocation.
- `Task.exit_reason` vaut `None` à l'initialisation. La première terminaison écrit la raison sous `run_queue_lock`, puis publie `Exited` en Release ; une terminaison ultérieure ne remplace pas la première valeur.
- Le contexte Ring 3 est référencé par un identifiant opaque. `attach_ring3_context` marque le slot CPU-affine avant de publier cet identifiant.
- `steal_task` ignore les slots CPU-affines. Le scénario QEMU SMP ajoute deux tâches marquées Ring 3 au test work-stealing ; les tâches noyau ordinaires restent éligibles au vol.
- `current_task_id`, `mark_current_exited` et `schedule_after_exit` constituent l'interface additive. Aucun chemin de faute ou syscall existant n'est encore branché sur ces API.

**Validation :** le contrat scheduler de terminaison Ring 3 a été fusionné dans `main` via la PR [#92](https://github.com/eemntirey/SYSTEMES/pull/92) (`6a77a50`). L'accesseur post-`Exited` `task_exit_reason` et la preuve QEMU de niveau 2 ont été fusionnés via la PR [#95](https://github.com/eemntirey/SYSTEMES/pull/95) (`2a720b8`). Sur le SHA final `97f8c48`, les 14 check-runs obligatoires étaient `success`, y compris `Ring 3 schedulable task termination` (QEMU/OVMF 1/2/4 CPU). Seul `Kilo Code Review` reste en `action_required`, ce qui n'est pas une condition de fusion selon les règles du dépôt.

## 10. P0 — terminaison d'une tâche Ring 3 ordonnançable (niveau 2, BSP)

**Date :** 2026-11-20  
**Feature :** `qemu-ring3-task-test`  
**Preuve locale :** QEMU/OVMF, `-smp 1`, `-smp 2` et `-smp 4`, sortie `isa-debug-exit` 0x10, `BOOT_PROOF_OK` ; 148 tests hôte et `cargo check --target x86_64-unknown-none` sans feature.

L'exécution Ring 3 est maintenant portée par une tâche de l'ordonnanceur. Le scénario crée trois tâches Ring 3, chacune avec son propre espace d'adressage, sa page de code et sa pile utilisateur :

| Tâche | Sortie | Raison publiée |
|---|---|---|
| 2 | `SYS_EXIT(42)` | `VoluntaryExit { code: 42 }` |
| 3 | lecture d'une page utilisateur non mappée | `PageFault { cr2, error_code=4, rip }` |
| 4 | `HLT` en CPL3 | `GeneralProtection { error_code=0, rip }` |

Une tâche noyau indépendante (slot 1) n'est déclarée réussie qu'après avoir observé les trois slots `Exited`. Cela prouve que le CPU a sélectionné une autre tâche après chaque terminaison. Chaque entrée écrit le CR3 privé observé dans un registre de test, et un canari utilisateur placé après l'instruction de sortie reste à zéro : le noyau n'a pas repris l'exécution utilisateur après `SYS_EXIT`, #PF ou #GP. Les espaces et leurs frames sont ensuite détruits et le compteur d'allocateur revient à sa valeur initiale.

Le chemin noyau enregistré par `enter_user_mode` est la frame de la fonction d'entrée de la tâche sur sa propre pile noyau. Après la sortie ou la faute, cette fonction publie la raison puis retourne à `task_bootstrap`, qui marque le slot `Exited` et cède le CPU. Aucune continuation utilisateur n'est reprise.

### Limites explicites (non acquises)

- Une seule tâche Ring 3 s'exécute à la fois par CPU : `gs_saved_kernel_rsp` reste CPU-local, donc deux tâches Ring 3 préemptées simultanément sur le même CPU écraseraient leur continuation. Le scénario désactive la préemption et chaque tâche Ring 3 termine avant la suivante. La pile d'entrée SYSCALL par tâche reste à introduire.
- Aucun reaper de production, aucun IPI TLB-shootdown ni masque de CPU actifs : la libération est faite par le harnais de test après retour au root noyau sur un seul CPU. Le protocole de réclamation SMP de la conception (PR D) n'est pas implémenté.
- Les tables de fichiers restent globales ; il n'y a ni processus multiples, ni `fork`, ni IPC.
- Le chemin fatal CPL0, les IST et le matériel physique restent inchangés et non validés sur machine réelle.
- Ces preuves ne valent que pour les SHA et configurations CI effectivement exécutés. Le job `ring3-task-exit` a confirmé la matrice 1/2/4 CPU sur le SHA final `97f8c48` (sortie `isa-debug-exit` 0x10, `BOOT_PROOF_OK`) ; la fusion est enregistrée sous `2a720b8`. Aucune validation sur matériel physique n'est revendiquée.

## 11. Clôture du jalon livré et suite du programme

**État au 21 novembre 2026 :** les niveaux 1 et 2 (BSP) de P0 sont intégrés dans `main` :
- PR #88/#89 : audit et récupération synchrone `#PF`/`#GP` CPL3 ;
- PR #91/#92/#95 : conception, contrat scheduler de terminaison et preuve QEMU de niveau 2.

Les limitations de la section 10 restent vraies : continuation noyau CPU-locale, absence de reaper/IPI TLB-shootdown, tables de fichiers globales, chemin CPL0 fatal. Le programme complet défini dans [P0_RING3_TASK_TERMINATION_DESIGN.md](P0_RING3_TASK_TERMINATION_DESIGN.md) se poursuit donc avec :
- **PR B** — propriété des tâches Ring 3, réservation/publication et activation (CR3, TSS.RSP0, GS/SYSCALL) ;
- **PR C** — trampoline de sortie non-retournant sur pile distincte, raccordement `SYS_EXIT` puis `#PF`/`#GP` ;
- **PR D** — reaper SMP, références, masque CPU actifs, IPI TLB-shootdown/ACK ;
- **PR E** — rafales, double terminaison, préemption, work-stealing noyau et build production.

Chaque incrément reste conditionné à une CI entièrement verte sur son SHA final et à un examen explicite avant fusion.
