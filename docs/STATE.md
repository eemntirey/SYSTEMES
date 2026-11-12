# État vérifié du dépôt SYSTEMES

**Audit du :** 2026-11-12  
**Branche auditée :** `main`  
**HEAD :** `a5c89e5d57bca9f874952d96ab801f98df1c27f1`  
**Commit :** `Implement multi-processus and PS/2 keyboard driver`

Ce document décrit l'état du code réellement présent sur ce SHA. « Compilé » signifie que le fichier appartient au graphe de modules Cargo et qu'une compilation CI a réussi ; un fichier Rust non déclaré dans `lib.rs` n'est pas considéré comme compilé.

## Synthèse

Le commit annonce des capacités qui ne sont pas livrées. Le noyau principal et ses tests existants passent la CI, mais cela ne valide pas les fichiers `process.rs` et `keyboard.rs` : ils ne sont pas exportés par `kernel/src/lib.rs`, donc Cargo ne les compile pas. Les appels système annoncés ne figurent pas dans `kernel/src/syscall.rs` et aucun test QEMU ne les exerce.

## Matrice de preuves

Légende : **Oui** = preuve observée sur le HEAD indiqué ; **Non** = absent/non validé ; **Partiel** = code préparatoire seulement.

| Composant | Implémenté | Compilé | Tests hôte auto | QEMU/OVMF | Preuve et constat |
|---|---|---|---|---|---|
| ABI syscall 0–8 (version, read/write, exit/yield, uptime, open/close/readdir) | Oui | Oui | Oui | Oui, pour les chemins marqués par la CI | Constantes et dispatch présents dans `syscall.rs`; CI #423 vérifie les contrats historiques |
| `SYS_SPAWN` | Non | Non | Non | Non | Aucune constante ni branche de dispatch ; `spawn_from_elf` existe uniquement dans un module non exporté |
| `SYS_FORK` | Non | Non | Non | Non | Aucune constante ni branche ; `fork_current()` retourne explicitement `Err("fork not yet implemented")` dans le fichier non exporté |
| `SYS_GETPID` | Non | Non | Non | Non | Aucune constante ni branche ; helper `getpid()` dans le fichier non exporté, non accessible via le graphe Cargo |
| `SYS_WAITPID` | Non | Non | Non | Non | Aucune constante, implémentation, état zombie conservé ou mécanisme d'attente/reaping identifié |
| Gestion de processus (`process.rs`) | Partiel | Non | Non | Non | Table statique, contexte et amorce spawn/yield/exit présents ; module absent de `lib.rs`, pas d'intégration démontrée dans le boot |
| `SYS_READKEY` | Non | Non | Non | Non | Aucune constante ni branche de dispatch |
| Pilote PS/2 (`keyboard.rs`) | Non — stub | Non | Non | Non | `init()` renvoie toujours `Ok("ok")`, `poll()` renvoie toujours 0 ; aucun accès contrôleur/clavier, décodage ou buffer d'événements |
| IRQ clavier / PIC IRQ1 / IDT 0x21 | Non | Non | Non | Non | Aucune intégration identifiée dans les sources exécutées ou les marqueurs CI |
| Ordonnanceur SMP P1-6c | Oui, selon les sources et tests existants | Oui | Oui | Oui | Job CI « SMP bring-up + BSP scheduler regression » et matrice QEMU 1/2/4 CPU verts sur ce SHA |
| Démarrage UEFI et shell UART | Oui | Oui | Oui | Oui | Jobs production boot sans disque et avec SYSFS verts |

## État de compilation et CI observé

Le workflow GitHub Actions #423 a terminé avec succès sur le SHA exact `a5c89e5d57bca9f874952d96ab801f98df1c27f1`, le 12 novembre 2026 :

- **13/13 check-runs verts**.
- Le job « no_std contract tests and x86_64 target check » a exécuté avec succès `cargo test --all-features --manifest-path kernel/Cargo.toml` et `cargo check --all-features --manifest-path kernel/Cargo.toml --target x86_64-unknown-none`.
- Les jobs de boot UEFI/OVMF, production shell, fautes matérielles QEMU, SMP bring-up et matrice SMP 1/2/4 CPU sont verts.
- Ces jobs ne prouvent pas les appels/processus/clavier annoncés : aucun job ne les compile ou ne vérifie leurs marqueurs dédiés. Le workflow ne construit pas un test Ring 3 de spawn/fork/wait ni un test d'entrée PS/2.

Cette CI valide les composants déjà raccordés au noyau ; elle ne valide pas les deux nouveaux fichiers non exportés.

## Tests manquants — critères d'acceptation

Ces tests sont **à écrire et à faire passer** après une véritable intégration des fonctions. Ils ne sont pas revendiqués comme exécutés.

### Processus et syscalls

1. Exporter le module processus dans le graphe Cargo, puis exécuter tests hôte et compilation `x86_64-unknown-none` afin que le fichier soit réellement compilé.
2. `SYS_SPAWN` : démarrer un ELF valide depuis VFS, vérifier PID unique, contexte initial, CR3 et entrée Ring 3 ; rejeter chemin absent, ELF invalide, table pleine et échec d'allocation sans fuite de frames.
3. `SYS_GETPID` : vérifier PID courant pour processus initial, enfant et après changement de contexte.
4. `SYS_FORK` : vérifier deux retours (parent PID enfant, enfant 0), copie indépendante des mappings/pages modifiables, héritage défini des descripteurs et nettoyage sur échec. Tant que la copie COW ou profonde n'est pas sûre, le syscall doit rester non exposé.
5. `SYS_WAITPID` : vérifier attente du PID ciblé, statut de sortie, enfant inexistant, enfant non terminé, réclamation exactement une fois et absence de zombie/fuite.
6. Tests d'intégration QEMU Ring 3 : spawn de deux ELF, yield/alternance, fork parent-enfant, exit avec statut, waitpid et vérification des marqueurs `SYSTEMES_TEST: ...`.
7. Vérifier que les pointeurs et longueurs passés par les nouveaux syscalls sont validés avant tout accès utilisateur et que les erreurs suivent les errno documentés.

### Clavier PS/2

1. Séparer le décodeur scan-code set 2 pur (testable hôte) du backend matériel.
2. Tests hôte : make/break, préfixes `0xE0` et `0xF0`, touches étendues, touches inconnues, séquences incomplètes, Shift/Caps Lock, buffer plein et débordement.
3. Initialisation matérielle : détection contrôleur 8042, délais bornés, ACK/RESEND, configuration du scan set, gestion du clavier absent et erreurs d'initialisation.
4. IRQ1 : routage PIC/IOAPIC cohérent avec le mode d'interruptions actif, lecture du port de données, EOI correct et aucune attente bloquante dans le handler.
5. QEMU/OVMF : injecter réellement des frappes clavier via le périphérique QEMU, vérifier les événements décodés et un syscall Ring 3 `SYS_READKEY` bloquant/non bloquant selon le contrat retenu.
6. Vérifier l'absence de perte silencieuse, de deadlock et de course entre le handler IRQ et le consommateur Ring 3.

### Règle de preuve

Un marqueur QEMU ne doit être émis qu'après l'opération réelle et la vérification de son résultat. Une injection clavier QEMU valide uniquement le périphérique émulé ; le matériel physique reste une preuve distincte.

## Conclusion

Le commit `a5c89e5` est une livraison trompeusement décrite : il ajoute deux fichiers préparatoires, mais ne livre pas les syscalls ni le pilote PS/2 annoncés. Les changements doivent être traités comme une nouvelle étape d'implémentation ciblée, après cette correction documentaire. Ne pas annoncer ces fonctions comme opérationnelles.
