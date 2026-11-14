# Conception P0 — terminaison d'une tâche Ring 3 ordonnançable

**Statut :** phase de conception uniquement. Aucun code noyau ni fichier de l'ordonnanceur n'est modifié dans cette PR.  
**Base examinée :** `main`, après fusion de la PR #89 (commit de fusion `a35c7aec23ce983c87adacf5fc24e5755d3316e8`).  
**Objectif :** après `SYS_EXIT` ou une faute CPL3 (#PF/#GP), abandonner définitivement le contexte Ring 3 courant, reprendre une autre tâche sur le même CPU, puis récupérer ses ressources sans libérer une pile ou une table de pages encore utilisée.

## 1. État réel du code et limites structurantes

- L'ordonnanceur est un ordonnanceur de tâches noyau par CPU : `Task`, `TaskResources` et `CpuScheduler` sont définis dans [scheduler.rs:22-124](../kernel/src/scheduler.rs#L22-L124). Chaque tâche possède déjà une pile noyau fixe de 16 Kio et un tampon FPU fixe ([scheduler.rs:13-20](../kernel/src/scheduler.rs#L13-L20), [scheduler.rs:82-92](../kernel/src/scheduler.rs#L82-L92)).
- Les entrées publiques actuelles sont `on_yield(saved_rsp)` et `on_timer(saved_rsp)`; toutes deux appellent `schedule` ([scheduler.rs:504-591](../kernel/src/scheduler.rs#L504-L591)). La fonction `schedule` sauvegarde la pile de la tâche courante et la rend Runnable si elle était Running ; elle ne peut donc pas être réutilisée telle quelle pour une sortie définitive sans distinguer l'état Exited ([scheduler.rs:504-575](../kernel/src/scheduler.rs#L504-L575)).
- `TaskState::Exited` existe déjà ([scheduler.rs:22-44](../kernel/src/scheduler.rs#L22-L44)), mais le chemin de sélection actuel ne fournit pas de primitive publique qui marque atomiquement la tâche courante Exited puis garantit un départ sans retour. `reset_after_all_tasks_exit` est un reset global local au CPU après retour au slot boot, pas une terminaison individuelle ([scheduler.rs:744-773](../kernel/src/scheduler.rs#L744-L773)).
- L'entrée Ring 3 actuelle est synchrone : `enter_user_mode(rip, rsp)` sauvegarde une continuation noyau et retourne après le trampoline ([interrupts.rs:964-1005](../kernel/src/interrupts.rs#L964-L1005)). Le trampoline restaure le RSP noyau sauvegardé et dépile les registres ([interrupts.rs:323-341](../kernel/src/interrupts.rs#L323-L341)). Ce mécanisme n'est pas une sauvegarde de contexte autonome exploitable par le scheduler.
- Les métadonnées de transition, de faute et de retour sont CPU-locales dans `SyscallCpuLocal` ([interrupts.rs:53-91](../kernel/src/interrupts.rs#L53-L91)). La faute #PF/#GP CPL3 est enregistrée, puis la frame est réécrite vers le trampoline de la continuation synchrone ([interrupts.rs:1210-1270](../kernel/src/interrupts.rs#L1210-L1270)). Ce chemin revient donc à l'appelant noyau ; il ne termine pas une tâche ordonnançable.
- `SYS_EXIT` ferme actuellement la table de fichiers globale, enregistre un statut CPU-local et fait sauter l'assembleur vers le même trampoline ([syscall.rs:334-342](../kernel/src/syscall.rs#L334-L342), [syscall.rs:368-425](../kernel/src/syscall.rs#L368-L425), [interrupts.rs:510-574](../kernel/src/interrupts.rs#L510-L574)). La table `PROCESS_FILES` est un singleton global verrouillé ([syscall.rs:37-40](../kernel/src/syscall.rs#L37-L40)) : elle ne constitue pas encore une propriété par processus.
- `AddressSpace` possède un PML4 et la liste des frames qu'il doit libérer ([address_space.rs:88-130](../kernel/src/address_space.rs#L90-L130)). Sa méthode `destroy` refuse uniquement de détruire le CR3 actif sur le CPU qui l'appelle ([address_space.rs:208-220](../kernel/src/address_space.rs#L208-L220)). Ce garde local n'est pas un protocole de shootdown SMP.
- Le test SMP Ring 3 existant partage un même root et utilise une pile virtuelle distincte par CPU ; chaque participant restaure son CR3 avant le rendez-vous de destruction ([address_space.rs:938-980](../kernel/src/address_space.rs#L938-L980), [address_space.rs:1050-1105](../kernel/src/address_space.rs#L1050-L1105)). Cela prouve un test synchronisé, pas la propriété d'un espace par tâche.
- L'ordonnanceur peut transférer une tâche Runnable entre CPU en transférant son couple pile/FPU ([scheduler.rs:178-269](../kernel/src/scheduler.rs#L178-L269)). Un contexte utilisateur ne doit pas être transféré tant que ses registres CPU-locaux, son TSS.RSP0 et son espace de pages ne sont pas migrables de façon sûre.

## 2. Modèle de propriété proposé

### 2.1 Propriétaire et données

Introduire un objet de contrôle utilisateur distinct du simple slot de queue. Le slot scheduler reste propriétaire de l'état d'ordonnancement et référence un contexte utilisateur stable, non déplaçable pendant toute la vie de la tâche.

Structure conceptuelle :

```rust
pub struct Ring3TaskContext {
    pub address_space_root: u64,
    pub user_rip: u64,
    pub user_rsp: u64,
    pub kernel_stack_top: u64,
    pub saved_kernel_rsp: u64,
    pub exit: Option<TaskExitReason>,
    pub lifecycle: Ring3Lifecycle,
}

pub enum Ring3Lifecycle {
    Prepared, Runnable, Running, Exited, Reaping, Reaped
}
```

Ceci est un contrat de conception, pas du code existant. L'objet propriétaire complet associe ce contexte à `AddressSpace`, à la pile utilisateur mappée dans cet espace, à la pile noyau épinglée et aux ressources de processus (dont une future table de fichiers par processus). Le scheduler ne libère jamais directement les frames de l'`AddressSpace` : il publie la fin de tâche et transfère la propriété à un reaper.

**Invariant :** un seul objet de tâche détient chaque `AddressSpace`, pile utilisateur et pile noyau. Les références publiées dans le scheduler restent stables jusqu'à `Reaped`. Aucun pointeur brut vers une pile ne survit à la libération de son propriétaire.

### 2.2 État CPU

Le contexte utilisateur persistant appartient à la tâche ; les registres d'entrée matérielle restent CPU-locaux. À chaque installation d'une tâche Ring 3 sur un CPU, le code d'activation devra synchroniser :
- `GS_BASE` sur le `SyscallCpuLocal` du CPU courant ([interrupts.rs:53-91](../kernel/src/interrupts.rs#L53-L91)) ;
- `TSS.RSP0` du CPU sur le sommet de la pile de privilège prévue pour l'entrée Ring 3 ; la TSS est intégrée à `CpuContext` ([interrupts.rs:35-117](../kernel/src/interrupts.rs#L35-L117)) ;
- la pile SYSCALL CPU-local comme pile d'entrée temporaire, puis la continuation noyau de la tâche courante ;
- l'identifiant et les informations de faute dans le contexte de tâche, pas uniquement dans `SyscallCpuLocal`.

Une tâche Ring 3 ne peut pas être migrée avec le mécanisme actuel de work-stealing : celui-ci ne déplace que le descripteur, la pile noyau et le FPU, sans reprogrammer les métadonnées SYSCALL/TSS/CR3 ([scheduler.rs:178-269](../kernel/src/scheduler.rs#L178-L269)). Pour la première livraison, les tâches Ring 3 sont donc **CPU-affines** ; la migration reste inchangée pour les tâches noyau.

## 3. Cycle de vie

1. **Création :** allouer un objet tâche stable, un `AddressSpace` privé, une pile utilisateur mappée U/S et une pile noyau épinglée ; préparer RIP/RSP et l'image de contexte. En cas d'échec partiel, annuler en ordre inverse sans publier la tâche.
2. **Publication :** sous le verrou de queue du CPU propriétaire, passer `Prepared -> Runnable` et publier le slot seulement lorsque toutes les ressources et métadonnées CPU nécessaires sont initialisées.
3. **Sélection :** le scheduler existant sélectionne une tâche Runnable en round-robin ; l'installation du contexte Ring 3 charge son CR3 et ses paramètres TSS/SYSCALL avant IRETQ. La politique round-robin reste inchangée ([scheduler.rs:504-575](../kernel/src/scheduler.rs#L504-L575)).
4. **Exécution :** timer/yield sauvegardent un contexte noyau complet sur la pile noyau de cette tâche et la rendent Runnable ; les interruptions ne doivent jamais enregistrer une continuation sur la pile d'une autre tâche.
5. **Sortie volontaire :** `SYS_EXIT` enregistre le code, ferme les ressources propres au processus, puis entre dans le même chemin noyau non-retournant que la faute. Le dispatcher actuel retourne `SYS_EXIT_DISPATCHED` et restaure la continuation synchrone ([syscall.rs:334-342](../kernel/src/syscall.rs#L334-L342), [syscall.rs:368-425](../kernel/src/syscall.rs#L368-L425), [interrupts.rs:510-574](../kernel/src/interrupts.rs#L510-L574)).
6. **Sortie forcée :** #PF/#GP dont le CS sauvegardé indique CPL3 enregistrent vecteur, code, CR2 pour #PF et RIP, puis demandent la terminaison de la tâche. Les fautes CPL0 restent fatales ; ne pas classer une faute sur le seul booléen `ring3_active` ([interrupts.rs:1210-1270](../kernel/src/interrupts.rs#L1210-L1270)).
7. **Réordonnancement :** marquer la tâche `Exited`, ne plus sauvegarder sa continuation comme Runnable, basculer sur une pile de sortie noyau sûre, puis choisir une autre tâche. Sans autre tâche Runnable, revenir à idle/boot.
8. **Réclamation :** après détachement du CR3 de tous les CPU et drainage des références lecteurs, passer `Exited -> Reaping -> Reaped`, détruire l'espace de pages et libérer piles/ressources une seule fois.

## 4. Terminaison par faute : pile et ordre impératif

Le niveau 1 utilise actuellement cette séquence : entrée sur la pile de privilège TSS ; sauvegarde des registres par `systemes_exception_common` ; le dispatcher remplace RIP/CS/RFLAGS et le RSP/SS sauvegardés pour viser `systemes_ring3_exit_trampoline` ; l'`iretq` revient en CPL0 ; le trampoline charge ensuite `saved_kernel_rsp` CPU-local ([interrupts.rs:323-341](../kernel/src/interrupts.rs#L323-L341), [interrupts.rs:636-704](../kernel/src/interrupts.rs#L636-L704), [interrupts.rs:1210-1270](../kernel/src/interrupts.rs#L1210-L1270)).

Pour une tâche ordonnançable, **ne pas reprendre la continuation de la tâche fautive**. Séquence cible :

1. Sur la pile de privilège/exception du CPU, valider le frame CPL3 ; copier les diagnostics dans le contexte de tâche ; réclamer atomiquement `Running -> Exited`. Cette pile est CPU-owned et n'est pas libérée par la terminaison.
2. Rediriger le retour matériel vers un trampoline noyau, mais **ne pas** y restaurer `saved_kernel_rsp` de la tâche fautive. Le trampoline bascule sur une pile de sortie/reaper noyau dédiée et stable (ou la pile idle du CPU), jamais sur la pile que l'on s'apprête à libérer.
3. Sous le verrou scheduler, retirer la tâche de l'ensemble sélectionnable, publier le prochain contexte et obtenir son RSP. Exited ne doit être ni réécrit Runnable ni sauvegardé par le chemin normal.
4. Installer le CR3, TSS.RSP0 et les champs CPU-locaux de la tâche suivante avant de lui rendre le contrôle. Le switch CR3 se produit depuis une pile noyau mappée dans les deux espaces.
5. Après avoir quitté la pile fautive et son CR3, mettre l'espace en attente de réclamation. Le reaper tourne sur idle ou une tâche noyau de fond.
6. Après confirmation SMP (aucun CPU n'utilise le root, aucun lecteur ne détient de référence), détruire `AddressSpace`, puis libérer pile utilisateur, pile noyau et autres ressources. Ordre : détacher de la queue -> basculer hors de l'espace -> attendre les acquittements -> libérer tables/mappings/frames -> libérer piles -> libérer le descripteur.
7. Si la terminaison survient pendant un switch, différer la réclamation : publier l'état immédiatement, mais ne reaper qu'après confirmation qu'aucun frame/CR3 actif ne référence la tâche.

**Règle absolue :** pile d'exception, pile de sortie et pile de tâche terminée sont trois rôles distincts. Aucune pile n'est libérée tant qu'un RSP matériel ou sauvegardé d'un CPU peut la référencer.

## 5. Espace d'adressage : choix et TLB

**Décision C1 : un espace d'adressage privé par tâche Ring 3 dès la première livraison fonctionnelle Ring 3**, même si ces tâches sont CPU-affines. Le root noyau reste partagé via le slot PML4 noyau, comme le fait déjà `AddressSpace::create` ([address_space.rs:90-130](../kernel/src/address_space.rs#L90-L130)) ; les mappings utilisateur et la pile utilisateur sont privés.

La CPU-affinité limite le processeur qui peut exécuter une tâche ; elle ne constitue pas une isolation mémoire et ne donne pas une propriété indépendante des mappings. Deux tâches affectées au même CPU peuvent s'exécuter successivement avec des images, piles, permissions et durées de vie distinctes. Avec un root utilisateur partagé, le retrait ou la libération des mappings d'une tâche pourrait invalider ou exposer ceux de l'autre. Le fait qu'un seul CPU utilise le root à un instant donné ne résout pas cette propriété logique ni l'isolation entre tâches.

Justification :
- Un root partagé, comme le test SMP actuel ([address_space.rs:938-980](../kernel/src/address_space.rs#L938-L980)), ne permet pas une isolation correcte entre tâches ayant images, piles ou permissions différentes.
- Un espace privé rend explicite la propriété des frames et permet la terminaison indépendante sans détruire les mappings d'une autre tâche.
- La CPU-affinité simplifie le suivi des CPU pouvant activer un root, mais ne remplace pas l'espace privé. Le coût est un CR3 par commutation Ring 3 et, si le root a pu être activé sur plusieurs CPU, un protocole TLB inter-CPU. C'est plus coûteux que le test synchrone, mais nécessaire pour la terminaison indépendante et l'isolation mémoire.

**État actuel :** `write_cr3` charge un root et invalide localement les entrées TLB non globales ([address_space.rs:357-365](../kernel/src/address_space.rs#L357-L365)) ; `AddressSpace::destroy` ne vérifie que le CR3 local ([address_space.rs:208-220](../kernel/src/address_space.rs#L208-L220)). Aucun mécanisme de shootdown TLB SMP ni registre des CPU actifs par address space n'a été trouvé dans le code audité. Il faut le construire avant toute libération concurrente d'un root potentiellement utilisé sur plusieurs CPU.

Protocole cible :
- Chaque espace maintient un masque atomique des CPU actifs (ou structure équivalente protégée) ; le bit est posé avant activation du CR3 et retiré après désactivation.
- À la destruction, empêcher toute nouvelle activation, snapshotter les CPU actifs, envoyer un IPI TLB-shootdown, exécuter `invlpg` ou recharger CR3 sur chaque CPU, puis attendre les ACK acquire/release.
- Le reaper est exécuté par le CPU propriétaire de la tâche, depuis son contexte idle ou un worker noyau local, jamais depuis la pile de la tâche terminée. Avec la règle CPU-affine stricte, aucun autre CPU ne doit activer ce root : après le switch local vers un CR3 sûr, le propriétaire retire le root de sa file de réclamation locale.
- Le reaper prend le verrou dédié de la file locale uniquement pour retirer un élément, puis le relâche avant toute destruction d'`AddressSpace` ou opération d'allocateur. Il ne prend pas `run_queue_lock` et ne prend jamais un verrou scheduler en attendant un autre CPU. Aucun spin-wait ni attente active n'est autorisé : si le masque CPU actif ou les références ne sont pas à zéro, l'élément est remis en attente et le worker poursuit d'autres tâches ; un mécanisme IPI/ACK asynchrone n'est requis que si une activation distante est constatée ou autorisée.
- Ordre de synchronisation : publication de la terminaison sous le verrou scheduler ; sélection et bascule hors du CR3 fautif ; transfert vers la file locale après confirmation du détachement ; retrait bref sous le verrou de file de réclamation ; relâchement de ce verrou ; vérification des références/CPU actifs ; destruction des tables et mappings ; libération des frames et piles ; libération finale du descripteur. Aucun verrou de file n'est conservé pendant la destruction.
- Pour le premier incrément scheduler, ne pas introduire de file de réclamation ni de mécanisme CR3 : ces responsabilités sont hors 4d. La CPU-affinité et l'interdiction de partager le root sont des invariants de la future activation Ring 3. Si un CPU distant peut avoir activé le root, le reaper diffère la libération sans attente active ; le protocole IPI/TLB-shootdown/ACK devra alors être implémenté avant toute réclamation.

Ce mécanisme et le reaper ne font **pas** partie de l'interface scheduler 4d ; ils sont des dépendances ultérieures, pas simulées par `AddressSpace::destroy`.

## 6. Concurrence, verrous et cas limites

### 6.1 Ordre des verrous

- `run_queue_lock` protège l'état et le slot scheduler ; les transferts existants prennent les verrous de deux CPU dans l'ordre croissant ([scheduler.rs:141-150](../kernel/src/scheduler.rs#L141-L150), [scheduler.rs:178-202](../kernel/src/scheduler.rs#L178-L202)).
- Règle proposée : **queue scheduler -> verrou de publication/réclamation du propriétaire de contexte**. Le chemin d'exception ne prend jamais un verrou VFS, allocator ou address-space en gardant le verrou de queue.
- Aucun chemin ne prend le verrou de propriété puis attend le verrou de queue. Le reaper ne tient aucun verrou scheduler pendant un shootdown ou une attente d'ACK.
- Les interruptions locales sont désactivées pendant les mutations de queue ([scheduler.rs:427-438](../kernel/src/scheduler.rs#L427-L438), [scheduler.rs:615-680](../kernel/src/scheduler.rs#L615-L680)). Ne pas attendre un IPI/CPU avec IF désactivé et un verrou dont ce CPU pourrait avoir besoin.

### 6.2 Préemption, IPI et timer

- Le timer appelle `on_timer` puis `schedule` si la préemption est activée ([scheduler.rs:582-591](../kernel/src/scheduler.rs#L582-L591)). La transition Exited doit être atomique vis-à-vis du timer : après publication Exited, aucune interruption ne remet la tâche Runnable.
- Les IPI de reschedule peuvent demander une nouvelle sélection, mais ne libèrent jamais un contexte. L'IPI de shootdown a une responsabilité distincte : invalider le TLB et acquitter.
- Pendant un switch, l'ancien et le nouveau propriétaire gardent une référence jusqu'à publication du nouveau CR3/RSP ; la réclamation est différée au-delà de cette fenêtre.
- Une tâche Ring 3 reste non-migrable dans cette livraison ; le timer peut la préempter sur son CPU, mais le work-stealing ignore ses slots.

### 6.3 Courses à traiter

| Cas | Règle |
|---|---|
| Faute pendant un changement de contexte | Utiliser le frame et la pile valides du CPU. Si l'identité de tâche n'est pas stable ou si le switch est critique, marquer une terminaison différée ; ne jamais libérer dans le handler. |
| Un autre CPU lit la tâche pendant sa terminaison | Le lecteur prend une référence/pin avant de déréférencer le contexte. Le reaper attend références à zéro et shootdown ACK. |
| Double terminaison (SYS_EXIT puis faute ou double événement) | Sous `run_queue_lock`, un seul gagnant écrit `exit_reason` puis publie `Exited` en Release. Les suivants observent Exited/Reaping/Reaped et ne modifient jamais la première raison ni ne publient une seconde réclamation. |
| Timer après Exited | Le chemin de sauvegarde reconnaît Exited et ne sauvegarde pas le RSP comme Runnable. |
| Tâche Ring 3 trouvée par work-stealing | Refuser le transfert tant que registres CPU-locaux, TSS et CR3 ne sont pas migrables. |
| Aucun autre Runnable | Revenir à idle/boot ; le reaper travaille ensuite depuis une pile distincte. |

Le `SpinLock` de la table VFS globale ne doit pas être pris depuis un handler de faute ; les handles doivent être détachés dans une étape normale différée et par processus. `SYS_EXIT` ferme aujourd'hui les handles avant le trampoline ([syscall.rs:334-342](../kernel/src/syscall.rs#L334-L342), [syscall.rs:368-425](../kernel/src/syscall.rs#L368-L425)) ; ce chemin devra devenir un nettoyage différé.

## 7. Interface scheduler minimale proposée (périmètre 4d)

Ne pas changer sélection round-robin, timer, priorités ou politique des tâches noyau. Ajouter uniquement :

### Types et champs

- `Ring3TaskContextRef` : identifiant opaque stable du contexte propriétaire ; pas un pointeur non protégé.
- `TaskExitReason` : enum `Copy`, sans allocation et à payload de taille fixe : `VoluntaryExit { code: i32 }`, `PageFault { cr2: usize, error_code: usize, rip: usize }` et `GeneralProtection { error_code: usize, rip: usize }`. L'option `None` signifie qu'aucune raison n'a été publiée.
- Dans `Task`, ajouter `ring3_context_id: AtomicUsize`, initialisé à `usize::MAX`, `cpu_affine: AtomicBool` initialisé à `false`, et `exit_reason: Option<TaskExitReason>` initialisé à `None`.
- Le champ `exit_reason` est écrit une seule fois sous `run_queue_lock`, avant la publication `Exited` avec ordre `Release`. Le gagnant vérifie l'état source puis écrit la raison et publie `Exited`; tout appel ultérieur qui observe `Exited`/`Reaping`/`Reaped` retourne sans toucher au champ. La raison n'est donc jamais remplacée.
- Le reaper observe d'abord `Exited` avec un chargement `Acquire`, puis lit `exit_reason` sans reprendre de verrou : la publication `Release` rend l'écriture visible et l'état terminal garantit qu'aucun écrivain ultérieur ne peut modifier le champ. L'ordre d'acquisition est donc uniquement `run_queue_lock` pour l'écriture ; le lecteur ne prend ni `run_queue_lock` ni verrou de reaper pour lire la raison. Le reaper acquiert séparément son verrou de file locale uniquement après le détachement CR3, conformément à C3 ; il ne garde jamais ce verrou pendant la lecture/ destruction du contexte.
- Les tâches noyau existantes conservent `exit_reason == None` tant qu'elles n'utilisent pas explicitement cette API ; leur initialisation et leur chemin de terminaison historiques ne sont pas modifiés.
- Ne pas placer `AddressSpace` ni des allocations dans `Task` : le scheduler conserve une référence/ID ; le propriétaire mémoire reste hors scheduler.

### Signatures exactes

```rust
pub fn attach_ring3_context(
    task: TaskId,
    context: Ring3TaskContextRef,
) -> Result<(), SchedulerError>;

pub fn current_task_id() -> Option<TaskId>;

pub fn mark_current_exited(
    reason: TaskExitReason,
) -> Result<(), SchedulerError>;

pub fn schedule_after_exit(saved_rsp: usize) -> usize;
```

Contrat :
- `attach_ring3_context` n'accepte qu'un slot initialisé non-boot, non courant, vide de contexte et dans l'état attendu ; il le rend CPU-affine avant de publier le contexte.
- `current_task_id` retourne le slot courant seulement si l'ordonnanceur est initialisé et l'index valide.
- `mark_current_exited` effectue sous `run_queue_lock` la transition idempotente : vérifier l'état, écrire `exit_reason` une seule fois, puis publier `Exited` en `Release`. Un seul appelant gagne ; une seconde terminaison ne peut ni remplacer la raison ni publier une nouvelle réclamation. Le contexte reste référencé pour le reaper.
- `schedule_after_exit` exige que la tâche courante soit déjà Exited ; il appelle la sélection existante sans sauver ni réactiver la tâche terminée. Il ne libère aucune ressource et retourne uniquement le RSP du prochain contexte.
- Le chemin timer/yield ordinaire garde son comportement actuel. Le scheduler ne fait ni switch CR3, ni manipulation TSS/GS, ni libération mémoire.

### Fichiers et zones visés en phase 2

- `kernel/src/scheduler.rs:22-44` : types d'état et erreurs ; type de raison de sortie.
- `kernel/src/scheduler.rs:57-80` : champs du descripteur `Task` et initialisation.
- `kernel/src/scheduler.rs:178-269` : filtrage CPU-affine dans work-stealing.
- `kernel/src/scheduler.rs:504-575` : chemin de sélection ; aucune modification de politique round-robin.
- `kernel/src/scheduler.rs:576-591` : nouvelles entrées publiques et conservation des points yield/timer.
- Tests unitaires dans `kernel/src/scheduler.rs` : états, double terminaison, refus migration, exclusion Exited.

**Limite assumée :** cette interface seule n'implémente pas l'entrée Ring 3 ordonnançable. Le raccordement CR3, TSS.RSP0, GS/SYSCALL, trampoline non-retournant, reaper et shootdown exige des changements hors scheduler. Ils ne sont pas autorisés par le périmètre 4d et feront l'objet de PR ultérieures explicitement approuvées.

## 8. Plan de tests QEMU — matrice 1 / 2 / 4 CPU

Les marqueurs ci-dessous sont des critères futurs, pas des résultats observés.

| Test | 1 CPU | 2 CPU | 4 CPU | Ce que cela prouve |
|---|---:|---:|---:|---|
| Création, exécution, `SYS_EXIT`, tâche B reprend après A | Oui | — | — | A ne reprend pas et le même CPU sélectionne B. |
| #PF puis #GP, chacune suivie de la reprise de B | Oui | — | — | Faute CPL3 -> Exited -> autre contexte, pas retour à la continuation fautive. |
| Deux tâches Ring 3 simultanées, une faute, les autres sortent normalement | — | Oui | Oui | Isolation des états de terminaison entre CPU. |
| Plusieurs tâches par CPU, alternance timer/yield | Oui | Oui | Oui | Préemption et round-robin existants restent fonctionnels. |
| Rafales création/terminaison avec compteurs frames et piles | Oui | Oui | Oui | Absence de fuite sur ressources gérées par le test. |
| Double terminaison / terminaison concurrente | Oui | Oui | Oui | CAS unique, pas de panic ni double libération. |
| Tâches noyau existantes, préemption, work-stealing SMP | Oui | Oui | Oui | Aucune régression du scheduler kernel. |
| CR3 détaché et ACK TLB avant reaping | — | Oui | Oui | Aucun frame/table libéré pendant qu'un CPU peut encore l'utiliser. |
| Build production sans features QEMU | N/A | N/A | N/A | Aucun changement de comportement ni dépendance test-only au boot. |

Chaque test doit d'abord être ajouté dans un état qui échoue réellement avant le correctif. Les logs série identifient CPU, TaskId, transition, motif de sortie et résultat de réclamation. Les compteurs avant/après incluent tables de pages, pile utilisateur, pile noyau et buffers FPU. Un marqueur « tâche terminée » seul ne prouve ni le changement de contexte ni la libération correcte.

## 9. Plan d'implémentation indépendant (3 à 5 PR)

1. **PR A — contrat scheduler (scope 4d seulement).** Tests unitaires tests-first pour attachement, marquage Exited idempotent, schedule-after-exit et exclusion work-stealing ; ajouter types/champs/API, sans CR3 ni code Ring 3. Risque : interaction avec la sauvegarde de `schedule`.
2. **PR B — propriété de tâche et activation Ring 3.** Objet propriétaire stable, espace privé et piles par tâche ; brancher CR3, TSS.RSP0 et métadonnées GS/SYSCALL à chaque switch. Ajouter d'abord le test d'une tâche Ring 3 et sortie volontaire. Risque élevé : continuation synchrone et états SYSCALL CPU-locaux.
3. **PR C — terminaison non-retournante.** Trampoline de sortie sur pile distincte, intégrer SYS_EXIT puis #PF/#GP, prouver sur un CPU la reprise d'une seconde tâche. Risque élevé : frame IRETQ, SWAPGS, RSP0 et durée de vie des piles.
4. **PR D — réclamation SMP.** Reaper, références/pins, masque CPU actif, IPI TLB-shootdown/ACK et destruction différée ; tests à 2 et 4 CPU, races et compteurs frames. Risque critique : ordre publication/ACK et deadlock IPI.
5. **PR E — régression et durcissement.** Rafales répétées, plusieurs tâches par CPU, double terminaison, work-stealing kernel, préemption/timer et build production sans features de test.

Chaque PR doit avoir sa CI entièrement verte sur le SHA final avant d'ouvrir l'incrément suivant. Maximum trois cycles correctif/push/CI par PR ; après trois échecs, rapport de blocage.

## 10. Build de production sans features de test

Tant qu'aucune tâche Ring 3 ordonnançable n'est créée, le chemin production conserve exactement le comportement actuel : aucune allocation d'espace utilisateur supplémentaire, aucun changement CR3, aucune activation du reaper/shootdown, aucune modification de politique round-robin kernel. Les types/API additifs peuvent être compilés sans être activés ; les chemins de test restent derrière leurs features. Le build production doit être exécuté et comparé dans la PR d'implémentation, pas présumé à partir des tests unitaires.

## 11. Décision de phase

Le niveau 2 est sensiblement plus large qu'un ajout de `TaskState::Exited` : il faut une continuation persistante, une pile de terminaison distincte, des métadonnées Ring 3 par tâche, une propriété privée d'espace de pages et un protocole de réclamation SMP. Le premier périmètre scheduler 4d peut être isolé et additif, mais ne doit pas être présenté comme une implémentation complète de la terminaison Ring 3.

**Point d'arrêt :** cette PR ne modifie que cette documentation. Aucun code d'ordonnanceur ne sera commencé avant accord explicite après revue de la conception.
