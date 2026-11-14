# État du noyau — suivi P0

**Actualisation :** 2026-11-14  
**Branche de référence :** `main`  
**Commit de référence :** `484c01e82568994bd9244e519839367f1ddc09cd` (squash de la PR #86)

Ce document décrit l'état intégré dans `main` après la PR #86. Les constats de l'audit du 13 novembre qui qualifiaient les états Ring 3/SYSCALL de globaux ne décrivent plus le code courant lorsqu'ils concernent les champs remplacés par le contexte CPU-local. La fermeture complète de P0 n'est toutefois pas acquise : la stabilisation complémentaire des transitions reste à auditer (section 6).

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
