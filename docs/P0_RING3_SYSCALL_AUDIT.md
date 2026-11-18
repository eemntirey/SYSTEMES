# Audit P0 — durcissement Ring 3 / SYSCALL par CPU

**Date :** 2026-11-14  
**Base vérifiée :** `main` — `98dfe4db12ab9817d48d4e7c83e0848477afe2da`  
**Branche :** `p0-ring3-syscall-hardening` (fast-forwardée sur cette base ; arbre identique à `main`)  
**Périmètre :** audit statique uniquement ; aucun code noyau modifié, aucun test désactivé.

| Point | Statut | Constat et preuve |
|---|---|---|
| a. STAR/LSTAR/SFMASK/EFER.SCE sur BSP/AP | **OK** | Le BSP initialise puis vérifie les MSR : [main.rs](../kernel/src/main.rs#L306-L327). Chaque AP exécute `initialize_syscalls()` et vérifie son slot : [smp.rs](../kernel/src/smp.rs#L650-L674). Écriture et vérification des quatre MSR : [interrupts.rs](../kernel/src/interrupts.rs#L823-L883). |
| b. Piles, TSS, RSP0/IST et LTR | **RISQUE conditionnel** | Chaque CPU possède un contexte, une pile d'entrée SYSCALL et des piles d'interruption dédiées ; RSP0 et IST1/2/3 sont configurés et `LTR` est exécuté dans `initialize_cpu` : [interrupts.rs](../kernel/src/interrupts.rs#L53-L117), [interrupts.rs](../kernel/src/interrupts.rs#L1000-L1069). Le chemin SYSCALL utilise une pile fixe par CPU, pas une pile par thread. Aucun modèle de threads Ring 3 multiplexés n'est actuellement établi ; cela deviendra bloquant avant leur introduction. La vérification explicite de TR/IST visible dans `main` couvre le BSP ; le code AP exécute bien `initialize_cpu`/LTR mais ne répète pas ces assertions de diagnostic. |
| c. SWAPGS syscall/IRQ/NMI/#MC/erreurs | **OK, couverture à étendre** | SYSCALL fait SWAPGS à l'entrée et à la sortie normale : [interrupts.rs](../kernel/src/interrupts.rs#L510-L556). IRQ vérifie le CPL et apparie le basculement avant `iretq` : [interrupts.rs](../kernel/src/interrupts.rs#L580-L632). L'entrée exception est paranoïaque et lit IA32_GS_BASE pour gérer les fenêtres NMI/MCE ; le marqueur contrôle le retour : [interrupts.rs](../kernel/src/interrupts.rs#L636-L704). Les erreurs non récupérables sont fatales (voir h). |
| d. Retour Ring 3, canonicalité et registres | **OK** | Le contexte de retour exige que RCX et RSP soient dans la fenêtre utilisateur : [syscall.rs](../kernel/src/syscall.rs#L490-L496), bornes définies dans [address_space.rs](../kernel/src/address_space.rs#L1-L30). Le chemin normal restaure les registres sauvegardés puis exécute SYSRETQ ; RAX porte le résultat et R11 est filtré : [interrupts.rs](../kernel/src/interrupts.rs#L510-L556), [syscall.rs](../kernel/src/syscall.rs#L364-L420). Les adresses de la fenêtre utilisateur sont canoniques par construction. |
| e. État mutable partagé sur le chemin syscall | **RISQUE conditionnel** | Les états de transition sont dans `SyscallCpuLocal` (par CPU) : [interrupts.rs](../kernel/src/interrupts.rs#L53-L91). En revanche, la table de descripteurs est un singleton global `PROCESS_FILES`, protégé par SpinLock : [syscall.rs](../kernel/src/syscall.rs#L25-L43), utilisé par OPEN/READ/CLOSE et vidée à la sortie : [syscall.rs](../kernel/src/syscall.rs#L100-L190), [syscall.rs](../kernel/src/syscall.rs#L270-L335). Le verrou évite une data race, mais pas le partage sémantique des descripteurs entre processus. À corriger avec l'introduction effective de plusieurs processus, pas par un changement spéculatif ici. |
| f. Pointeurs utilisateur et SMAP | **OK** | Les plages sont bornées avec addition vérifiée : [uaccess.rs](../kernel/src/uaccess.rs#L65-L90). Les handlers vérifient les plages et mappings avant lecture/écriture ; les accès sont entourés de `UserAccessGuard` : [syscall.rs](../kernel/src/syscall.rs#L100-L190), [syscall.rs](../kernel/src/syscall.rs#L440-L490), [uaccess.rs](../kernel/src/uaccess.rs#L125-L185). Le garde exécute STAC/CLAC. |
| g. Fenêtres IRQ/IPI à l'entrée SYSCALL | **OK dans le chemin vérifié** | SFMASK masque IF, TF, DF, IOPL, NT, RF et AC : [interrupts.rs](../kernel/src/interrupts.rs#L823-L833). L'entrée installe GS et la pile avant l'appel Rust : [interrupts.rs](../kernel/src/interrupts.rs#L510-L530). Le handler qui réactive explicitement les IRQ, SYS_READ UART, remasque IF avant le retour : [syscall.rs](../kernel/src/syscall.rs#L440-L485). Le test SMP actuel exige des interruptions timer depuis Ring 3 : [address_space.rs](../kernel/src/address_space.rs#L870-L960). |
| h. #PF/#GP Ring 3 et isolation des autres CPU | **RISQUE confirmé (limite fonctionnelle)** | Seul INT3 est repris ; les autres exceptions, dont #PF/#GP, terminent dans `cli; hlt` sur le CPU fautif : [interrupts.rs](../kernel/src/interrupts.rs#L1140-L1240). Aucun mécanisme de terminaison/signal du thread utilisateur n'existe dans ce chemin. Le code ne démontre pas de corruption d'un autre CPU, mais ne tue/signale pas le thread fautif comme demandé. |

## Tests de non-régression existants

- Le test QEMU SYSCALL couvre GET_API_VERSION, SYS_WRITE/SYS_READ avec adresse invalide, syscall inconnu, YIELD et EXIT : [address_space.rs](../kernel/src/address_space.rs#L532-L650).
- Les tests unitaires couvrent les bornes utilisateur, les pointeurs hors plage et la disposition du frame : [syscall.rs](../kernel/src/syscall.rs#L510-L610).
- Le test SMP exécute simultanément un participant Ring 3 par CPU, avec piles distinctes, un syscall GET_API_VERSION, deux chemins de sortie et vérification du timer APIC : [address_space.rs](../kernel/src/address_space.rs#L740-L1020).

## Tests QEMU complémentaires recommandés avant tout correctif

1. **Rafale par CPU :** compléter le syscall unique du test SMP par une boucle de plusieurs milliers d'appels GET_API_VERSION/YIELD sur chaque participant ; conserver compteurs et résultats distincts par CPU ; exécuter la matrice 1/2/4 CPU.
2. **Pointeurs adversariaux :** ajouter aux cas QEMU les adresses non canoniques, additions débordantes, pages absentes et destinations non inscriptibles, en vérifiant le code d'erreur sans marqueur fatal noyau.
3. **Préemption pendant transition :** répéter les allers-retours Ring 3/SYSCALL pendant les interruptions timer et, si un IPI de reschedule est disponible dans le scénario, pendant les IPI ; contrôler les registres sentinelles et l'invariant GS après chaque retour.
4. **Faute Ring 3 sur un CPU :** scénario multi-CPU où un participant déclenche volontairement #PF/#GP tandis que les autres continuent leurs syscalls. Le test doit d'abord constater le comportement actuel (CPU fautif immobilisé, pas de terminaison de tâche), puis servir de test de non-régression lorsque le modèle de tâches utilisateur permettra une terminaison récupérable. Ne pas assimiler un simple `hlt` à une terminaison correcte.
5. **Isolation des descripteurs :** seulement lorsque plusieurs processus utilisateur sont réellement pris en charge : ouvrir le même numéro de descripteur dans deux processus, vérifier l'indépendance des tables et qu'une sortie ne ferme pas les handles de l'autre.

## Conclusion

**Aucun BUG CONFIRMÉ dans le protocole actuel SYSCALL/SWAPGS ni dans les validations de pointeurs examinées.** Deux risques d'architecture conditionnels (pile SYSCALL par thread et table de fichiers globale) précèdent l'introduction de threads/processus utilisateurs multiples ; une limite fonctionnelle est confirmée pour les fautes Ring 3 (#PF/#GP), actuellement fatales au CPU fautif.

Aucun correctif noyau n'est justifié dans cette étape d'audit. Les tests complémentaires ci-dessus définissent les critères de validation avant toute évolution. **Aucun fichier d'ordonnanceur n'a été modifié.** Aucun build QEMU n'a été exécuté pendant cet audit statique.


## 8. Suivi — récupération synchrone Ring 3 (PR #89)

La phase de récupération synchrone traite uniquement les fautes #PF (14) et #GP (13) dont le CS sauvegardé indique CPL3. Le dispatcher conserve les informations vecteur, code d'erreur, CR2 (#PF) et RIP fautif dans le contexte CPU-local, puis redirige la frame vers la continuation noyau sauvegardée. Les exceptions CPL0 restent sur le chemin fatal ; la valeur ring3_active n'est jamais utilisée seule pour classer le CPL.

Les tests ajoutés dans la branche p0-ring3-fault-handling couvrent :
- un #PF sur page utilisateur non mappée et un #GP via HLT exécuté en CPL3 ;
- dix entrées fautives répétées sur la même continuation ;
- la poursuite du scénario Ring 3/SYSCALL des autres CPU participants ;
- le rendez-vous avant destruction de l'espace d'adressage partagé et l'égalité du nombre de frames libres avant/après ;
- un test séparé des pointeurs invalides passés à SYS_READ/SYS_WRITE, qui doivent retourner ERROR_BAD_ADDRESS sans tuer la transition.

**État : implémentation présente sur la branche, validation QEMU/CI en attente.** Ne pas interpréter les marqueurs attendus dans les scripts comme des résultats déjà observés. Le niveau 2 (terminaison d'une tâche Ring 3 ordonnançable) n'est pas traité ; voir [P0_RING3_TASK_TERMINATION_DESIGN.md](P0_RING3_TASK_TERMINATION_DESIGN.md).

IST1 (#DF), IST2 (NMI) et IST3 (#MC) ne sont pas modifiées. Une exception imbriquée depuis CPL0 reste fatale. Aucun fichier de scheduler ne doit être modifié dans cette phase.


**Test CPL0 non exécuté :** le gestionnaire fatal actuel exécute `cli; hlt` sur le CPU fautif. Injecter volontairement une faute noyau dans la matrice SMP arrêterait ce CPU avant le rendez-vous et le marqueur `--debug-exit` ; cela ne permettrait pas de distinguer proprement le comportement fatal du blocage de la matrice. Une preuve dédiée nécessiterait un démarrage QEMU isolé et un point d'arrêt/debug-exit réservé à cette configuration de test. Le chemin de production reste fatal et n'est pas modifié.


## 9. Suivi — contrat scheduler de terminaison Ring 3 (incrément 1)

La branche `p0-ring3-task-termination` introduit une interface scheduler additive pour préparer la terminaison d'une tâche Ring 3 ordonnançable : attachement d'un identifiant opaque de contexte, lecture du TaskId courant, publication d'une raison de sortie unique et sélection après `Exited`.

`TaskExitReason` couvre les sorties volontaires, #PF (CR2/code/RIP) et #GP (code/RIP) avec des payloads fixes et sans allocation. Le champ `Task.exit_reason` reste `None` par défaut. L'écriture est sérialisée sous `run_queue_lock`, avant la publication Release de `Exited`; le lecteur post-Exited acquiert l'état puis lit la valeur immuable sans verrou additionnel. La double terminaison ne remplace pas la première raison.

Les slots associés à un contexte Ring 3 sont CPU-affines et exclus du work-stealing. Le scénario SMP ajouté doit démontrer ce comportement sur 2 et 4 CPU pendant que les AP inactifs tentent de voler les tâches ; il n'est pas encore considéré comme réussi avant CI.

**Limite :** aucune entrée #PF/#GP, aucun SYS_EXIT, aucun switch CR3/TSS/GS et aucune libération/reaper ne sont raccordés par cet incrément. Le constat de l'audit selon lequel les fautes Ring 3 ne terminent pas encore une tâche ordonnançable reste donc vrai jusqu'aux incréments ultérieurs.
