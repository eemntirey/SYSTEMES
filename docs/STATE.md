# État du noyau — audit de concurrence P0

**Date de l'audit :** 2026-11-13  
**Base auditée :** `main`  
**SHA de base :** `a5c89e5d57bca9f874952d96ab801f98df1c27f1`  
**Branche de cet audit :** `p0-concurrency-audit`

Ce document sépare l'état réellement intégré dans `main` des modifications encore proposées dans des PR. L'audit est une inspection statique du code GitHub. Il ne constitue ni une compilation locale, ni un test QEMU, ni une preuve de sécurité SMP.

## Verdict P0

<badge>NON VALIDÉ</badge>

Le noyau ne peut pas encore être déclaré sûr pour l'exécution concurrente de processus Ring 3 et d'appels SYSCALL sur plusieurs CPU.

Les blocages critiques sont :
1. La table de processus et l'allocateur de frames sont des `static mut` exposés par des fonctions qui rendent des références mutables à durée de vie `'static`, sans verrou ni propriété exclusive démontrée.
2. Le PID courant est un unique atomique global, et non un état local au CPU. Il ne représente donc pas correctement plusieurs processus exécutés simultanément.
3. La PR #78 rend la pile et les données d'entrée SYSCALL locales au CPU, mais conserve des états de sortie Ring 3 globaux.
4. Le code SYSCALL utilise une table de descripteurs globale alors que `Process` possède déjà une table par processus. Les chemins syscall n'utilisent pas `current_file_table()`.
5. Les tests SMP actuels vérifient notamment le démarrage des AP, des compteurs atomiques/verrouillés et la migration de tâches. Ils ne démontrent pas des appels SYSCALL Ring 3 concurrents sur 2 et 4 CPU.

Aucune expansion fonctionnelle ne doit être ajoutée pendant la fermeture de ces blocages.

## 1. Inventaire des `static mut` dans `main`

Les déclarations ci-dessous ont été retrouvées dans les sources de `kernel/src` sur le SHA de base. Leur présence dans un fichier ne signifie pas nécessairement que ce fichier appartient au graphe Cargo : `process.rs` n'est pas déclaré dans `kernel/src/lib.rs` sur cette base.

| Fichier / symbole | Rôle | Accès / garantie observée | Verdict P0 |
|---|---|---|---|
| `heap.rs::HEAP_STORAGE` | Arène statique du tas | Adresse obtenue pendant l'initialisation de `GLOBAL_ALLOCATOR`; l'état du tas est protégé par le verrou interne `KernelHeap`. | Justification présente ; vérifier qu'aucun accès direct concurrent ne contourne le verrou. |
| `process.rs::PROCESS_TABLE` | Table de processus | `process_table()` retourne `&'static mut ProcessTable` sans synchronisation. | **Critique — non sûr.** |
| `process.rs::FRAME_ALLOCATOR` | Allocateur de frames | `frame_allocator()` retourne `Option<&'static mut PhysicalFrameAllocator>` depuis un global mutable sans verrou. | **Critique — non sûr.** |
| `smp.rs::SYSTEMES_AP_STACKS` | Pointeurs de piles AP consommés avant Rust | Écrit par le BSP et lu par le trampoline AP ; la sûreté dépend de la publication avant SIPI et de l'ordre de démarrage. | À justifier formellement par le protocole release/acquire et le code assembleur. |
| `interrupts.rs::CPU_CONTEXTS` | GDT/IDT/TSS et piles d'interruption par CPU | Indexé par slot CPU ; `CPU_INIT_STATE` utilise un compare-exchange pour réserver le slot et un état publié à la fin de l'initialisation. | Modèle par slot plausible ; vérifier tous les lecteurs et l'unicité APIC ID → slot. |
| `interrupts.rs::SYSTEMES_SYSCALL_KERNEL_STACK` | Pile d'entrée SYSCALL historique | Une pile unique partagée. | **Critique — incompatible SMP.** Remplacée dans la branche de la PR #78, pas dans `main`. |
| `interrupts.rs::SYSTEMES_SYSCALL_USER_RSP` | RSP utilisateur sauvegardé à l'entrée SYSCALL | Un emplacement global partagé par tous les CPU. | **Critique — corruption possible entre appels concurrents.** Remplacé dans la branche de la PR #78. |
| `interrupts.rs::SYSTEMES_RING3_SAVED_KERNEL_RSP` | RSP noyau sauvegardé lors de l'entrée Ring 3 | Une seule valeur globale lue par le chemin de sortie d'interruption. | **Critique — non isolé par CPU/processus**, y compris dans la branche #78. |
| `scheduler.rs::CPU_SCHEDULERS` | Files et métadonnées d'ordonnanceur par CPU | Pointeurs bruts vers un slot indexé par CPU ; le code emploie un verrou de file par CPU et plusieurs champs atomiques. | Protection partielle ; audit des accès hors verrou et des migrations encore requis. |
| `scheduler.rs::TASK_RESOURCES` | Piles noyau et buffers FXSAVE des tâches | Pointeurs bruts vers des ressources stables ; commentaire de code décrit un transfert de propriété lors des migrations. | Invariant de propriété à documenter et tester ; pas une preuve suffisante à lui seul. |

## 2. État partagé supplémentaire (pas nécessairement `static mut`)

| Symbole | Protection / usage | Risque restant |
|---|---|---|
| `process.rs::CURRENT_PID` | `AtomicU32` avec Acquire/Release. | Atomique mais global : un seul PID courant pour tous les CPU. Doit devenir CPU-local ou être dérivé du contexte CPU courant. |
| `interrupts.rs::RING3_ACTIVE` | `AtomicBool` global. | L'atomicité n'isole pas les sessions Ring 3 : un CPU peut modifier l'état observé par un autre. |
| `interrupts.rs::RING3_EXIT_VALIDATED` | `AtomicBool` global. | Même problème d'isolation. |
| `interrupts.rs::RING3_EXPECTED_USER_RSP` | `AtomicU64` global. | Un test Ring 3 concurrent peut valider le mauvais RSP. |
| `interrupts.rs::SYSTEMES_SYSCALL_EXIT_STATUS` | `AtomicU64` global exporté pour l'assembleur. | Statut de sortie partagé ; doit être CPU-local ou rattaché au contexte de processus. |
| `syscall.rs::PROCESS_FILES` | `SpinLock<ProcessFileTable>`. | Pas de data race sur la table, mais sémantique globale incompatible avec l'isolation des FD par processus. |
| `rootfs.rs::ROOT` | `SpinLock<Option<Vfs<'static>>>` ; accès via `with_root`. | Accès sérialisé ; documenter les règles d'ordre des verrous et éviter les appels imbriqués non maîtrisés. |
| `heap.rs::GLOBAL_ALLOCATOR` | `KernelHeap` avec verrou atomique et `UnsafeCell`. | Protection annoncée dans le code ; attention aux allocations pendant qu'un verrou non réentrant est détenu. |
| Compteurs AP / horloge / APIC | Principalement atomiques. | Vérifier que chaque variable globale représente bien un compteur partagé, et non un état qui devrait être local au CPU. |

## 3. Table de processus et allocateur — branche de la PR #82

La PR #82 (`test/process-context-contracts`) ajoute `pub mod process` dans `lib.rs`, ce qui fait entrer `process.rs` dans le graphe Cargo. Elle ne corrige pas les accès globaux.

Constats dans cette branche :
- `process_table()` fabrique une référence mutable globale sans verrou.
- Les fonctions de création, sélection, changement de processus, sortie et accès à la table des fichiers appellent directement `process_table()`.
- `frame_allocator()` peut fabriquer plusieurs références mutables aliasées vers le même allocateur.
- `current_file_table()` retourne lui aussi une référence mutable à durée de vie `'static` dérivée de la table globale.
- `CURRENT_PID` est un unique atomique global ; il ne peut pas représenter les PID courants de plusieurs CPU.
- Les tests ajoutés dans #82 couvrent le round-trip des registres, l'état initial de la table et les discriminants d'état. Ils ne couvrent pas l'accès concurrent à la table, l'allocateur, le changement de CR3 ou le scheduling Ring 3.

**Conclusion :** ne pas intégrer #82 telle quelle dans une base qui prétend prendre en charge plusieurs processus Ring 3 simultanés.

## 4. Descripteurs de fichiers — double modèle

Dans `syscall.rs`, les chemins `open`, `read`, `readdir`, `close` et la fermeture à la sortie utilisent `PROCESS_FILES`, une table globale protégée par `SpinLock`.

Dans `process.rs`, chaque `Process` possède déjà `file_table: ProcessFileTable`, et des fonctions `current_file_table()` / `close_current_process_files()` ciblent cette table par processus. Les chemins syscall ne s'appuient pas sur ces fonctions.

Le verrou global empêche une corruption concurrente de la structure globale, mais ne garantit pas l'isolation :
- deux processus partageraient le même espace de descripteurs ;
- les numéros de FD ne seraient pas indépendants ;
- la fermeture globale pourrait fermer les handles d'un autre processus ;
- la table par processus resterait inutilisée par les syscalls.

Correction attendue : une seule source d'autorité par processus, accès contrôlé à partir du contexte de processus courant, et fermeture limitée aux handles de ce processus. Ne pas conserver deux tables concurrentes.

## 5. Entrée SYSCALL et sortie Ring 3 — PR #78 / #84

### PR #78 — `p1-6d-per-cpu-syscall-entry`

La branche ajoute :
- un `SyscallCpuLocal` indexé par CPU avec `kernel_stack_top` à l'offset 0 et `user_rsp` à l'offset 8 ;
- une pile noyau SYSCALL par CPU ;
- `SWAPGS` à l'entrée et à la sortie SYSRET ;
- la programmation de GS_BASE / KERNEL_GS_BASE et des MSR SYSCALL sur BSP et AP.

Mais les symboles suivants restent globaux dans cette branche :
- `SYSTEMES_RING3_SAVED_KERNEL_RSP` ;
- `RING3_ACTIVE` ;
- `RING3_EXIT_VALIDATED` ;
- `RING3_EXPECTED_USER_RSP` ;
- `SYSTEMES_SYSCALL_EXIT_STATUS`.

La pile et le RSP d'entrée deviennent locaux au CPU ; le protocole de sortie Ring 3, lui, ne l'est pas. Le contrat actuel suppose également que le GS utilisateur vaut zéro : l'utilisation ultérieure de GS côté userland/TLS devra être conçue explicitement.

### PR #84 — `p1-7a-syscall-entry-contracts`

Cette PR ajoute des tests de layout pour les offsets GS [0] et [8]. Ces tests valident un contrat mémoire, pas la sûreté de l'entrée/sortie concurrente. La PR est empilée sur #78 et ne doit pas être fusionnée indépendamment de sa parente.

**Conclusion :** #78 et #84 restent des travaux préparatoires ; leurs tests de layout ne ferment pas les blocages de concurrence Ring 3.

## 6. Verrous et ordre d'acquisition

- `SpinLock` sérialise les données qu'il protège, mais n'offre pas de détection d'inversion de verrous ni de réentrance.
- `rootfs::with_root` acquiert le verrou `ROOT` pendant l'exécution de sa closure.
- Les syscalls actuels prennent généralement une copie du handle sous `PROCESS_FILES`, libèrent ce verrou, puis appellent `with_root`. L'inventaire ne montre donc pas, dans ces chemins précis, une acquisition imbriquée systématique des deux verrous.
- Le chemin de sortie de processus de la branche #82 manipule la table de processus puis appelle le VFS pour fermer les handles ; la table de processus n'est pas protégée par un verrou aujourd'hui. Une future correction devra définir un ordre unique et éviter de conserver un verrou de table de processus pendant des opérations VFS/allocation potentiellement bloquantes.
- L'allocateur de frames ne doit pas être appelé via une référence mutable globale non exclusive. L'ordre entre verrou de processus, allocateur, heap et VFS doit être explicite avant de remplacer les globals par des verrous.

**Limite de l'audit :** l'analyse statique des chemins identifiés ne remplace pas un audit dynamique de tous les entrelacements. Aucun ordre global de verrous n'est actuellement prouvé par des assertions ou un outil de vérification.

## 7. État des preuves et critères de sortie

| Contrôle P0 | État constaté | Preuve requise pour clôture |
|---|---|---|
| Inventaire des `static mut` | Inventaire initial de `main` établi ; branches PR distinguées. | Refaire l'inventaire sur le SHA final après corrections. |
| Références mutables globales dangereuses | Plusieurs identifiées, notamment table de processus et allocateur. | Zéro accès non justifié ; API sans références mutables globales aliasables. |
| Contexte GS | Entrée per-CPU proposée dans #78 ; sortie Ring 3 encore globale. | Isolation complète et testée sur chaque CPU. |
| SYSCALL concurrent | Non démontré. | Test Ring 3 qui exécute des syscalls simultanément sur 2 et 4 CPU et vérifie les contextes/compteurs indépendants. |
| SMP 1 / 2 / 4 CPU | Des tests SMP et une matrice QEMU existent dans le workflow. | Exécution réussie sur le SHA final ; les tests existants ne suffisent pas à prouver les syscalls concurrents. |
| Tests de concurrence processus | Insuffisants. | Tests concurrents de la table, de l'allocateur, des FD et des changements de contexte. |
| Documentation | Ce document crée la base de suivi P0 sur cette branche. | Actualiser les preuves, les SHA et les résultats après chaque correction. |
| CI sur SHA final | Non vérifiée pour cette branche. | Tous les checks requis verts sur le HEAD exact de la PR finale. |

## 8. Plan de correction strictement limité à P0

1. Finaliser le modèle CPU-local du contexte SYSCALL et des états de sortie Ring 3 ; ne pas créer une deuxième implémentation concurrente de #78.
2. Remplacer les références mutables globales de la table de processus et de l'allocateur par des API synchronisées à durée de vie bornée (verrou/guard ou closures), sans exposer de `&'static mut`.
3. Remplacer le PID courant global par un état CPU-local cohérent avec le contexte GS/per-CPU.
4. Unifier la gestion des FD sur la table du processus courant ; supprimer la table globale syscall après migration et tests, pas avant.
5. Formaliser l'ordre des verrous et la propriété des ressources pendant spawn, yield, exit et migration.
6. Ajouter les tests concurrents et les scénarios QEMU SYSCALL Ring 3 sur 1, 2 et 4 CPU.
7. Mettre à jour ce fichier avec le SHA testé, les logs QEMU et les résultats CI. Aucun merge avant CI entièrement verte sur le SHA exact.

Aucune implémentation de fork complet, extension POSIX, VFS inscriptible, ELF avancé ou nouveau pilote n'entre dans cette mission.
