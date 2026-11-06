# SYSTEMES — Plan des prochaines étapes

Date : 6 novembre 2026
Branche courante : `feat/windows-real-desktop` (à jour sur `origin`, 2 commits en avance sur `main`)
Application : édition Desktop/Host Windows (WPF + WebView2 + moteur .NET 8) — **build Release lancé et fonctionnel**

---

## 0. État réel constaté aujourd'hui

| Domaine | État |
|---|---|
| Branche locale | Synchronisée avec `origin/feat/windows-real-desktop` (`git pull` → *Already up to date*) |
| Build frontend | `npm run build` (typecheck + Vite) : ✅ 0 erreur |
| Build natif | `dotnet build ... -c Release` : ✅ 0 warning / 0 erreur (host + moteur natif publié) |
| Lancement réel | `SYSTEMES.WindowsHost.exe` (PID 12244) : bureau, Paramètres, barre des tâches rendus dans WebView2 |
| CI (GitHub Actions) | `ci.yml` (typecheck/tests/build + host Windows + smoke JSON + smoke UI WebView2) et `kernel.yml` (QEMU/OVMF) |
| Issues ouvertes | aucune |
| PR ouvertes | 17 (dont une pile empilée K2.4 → K2.11, et 2 PR documentaires probablement obsolètes) |

### Travail local non committé (à traiter en premier)
- `package.json` : champs `description` et `license` ajoutés.
- `dist/index.html` : régénéré par le build.
- Non suivis : `LICENSE`, `README.md`, `_tmp_parse_prs.py` (script jetable, à supprimer).

### Avancement par piste
- **Piste Windows (K3)** : K3.1.1 ✅, K3.1.2 ✅ → prochain chantier **K3.1.3**.
- **Piste noyau (K2)** : K2.1 → K2.3 et K2.6.4 (PS/2) déjà sur `main` ; la pile K2.4 → K2.11 est **entièrement empilée et mergeable**, non fusionnée.
- **Piste docs** : OBSIDIAN shell/start menu sur `main` ; Explorateur (PR #27) et docs (#29, #37) en attente.

---

## 1. Étape immédiate (aujourd'hui) — hygiène et lancement

| # | Action | Critère de succès |
|---|---|---|
| 1.1 | Committer les changements locaux : `LICENSE`, `README.md`, champs `package.json` ; régénérer ou exclure `dist/` du suivi ; supprimer `_tmp_parse_prs.py` | `git status` propre |
| 1.2 | Ouvrir une PR `feat/windows-real-desktop` → `main` (2 commits : partages réseau/lecteurs logiques/infos système + lancement sandboxé/lecteurs réseau/propriétés fichiers) | CI verte sur la PR |
| 1.3 | Confirmer que l'édition native se lance sans Vite (`npm run build` → `dotnet build` → exe) | Fenêtre SYSTEMES affichée, console sans erreur |

## 2. Étape courte (cette semaine) — débloquer la file de PR

| # | Action | Détail |
|---|---|---|
| 2.1 | **Fermer les PR obsolètes** : #12 (K2.2) et #14 (K2.3) | leur contenu est déjà sur `main` via #13/#15 (vérifié : `kernel/` identique pour #12) |
| 2.2 | **Décider du sort de #8** (Phase 3 Windows Update/Paramètres) | largement remplacé par #9/#10 → rebase et revue, ou fermeture |
| 2.3 | **Débloquer le check `Kilo Code Review` = `action_required`** qui met les PR en état `unstable` | approuver/rerunner la revue sur #16, #27, #29, #35, #37, #39 |
| 2.4 | Laisser finir le job en cours sur #39 (`x87/SSE context isolation across preemptive tasks` = in_progress) | check vert avant fusion |
| 2.5 | Fusionner la PR docs **#37** (roadmap/audit) et **#29** (mission gratuite) | docs à jour sur `main` |

## 3. Étape structurante (1–2 semaines) — fusionner la pile noyau K2

Pile empilée, toutes mergeable, CI QEMU verte de bas en haut :

```
#16 K2.4  protection mémoire (NX, W^X)         base: main
#17 K2.4.1 tests matériels NX/W^X
#18 K2.5   TSS, IST, piles d'exceptions
#19 K2.6   PIC 8259, IRQ matérielles, PIT
#20 K2.6.1 découverte ACPI/MADT, Local APIC
#21 K2.6.2 xAPIC + routage IRQ0 par IOAPIC
#35 K2.7   ordonnanceur préemptif
#36 K2.8   contexte FPU/SIMD
#38 K2.9   horloge Local APIC calibrée
#39 K2.10  heap dynamique
  └─ #40 K2.11 inventaire SMP (déjà fusionné dans la pile)
```

Ordre de fusion : **#16 → #17 → #18 → #19 → #20 → #21 → #35 → #36 → #38 → #39**, une PR à la fois, en revalidant la CI après chaque fusion (les bases sont des branches de la pile : après fusion de #16, rebaser/régénérer #17, etc.).

Critère de sortie : `main` contient K2 complet (mémoire protégée, interruptions matérielles, APIC, ordonnanceur préemptif, FPU, heap, inventaire SMP) avec pipeline vert.

## 4. Étape suivante du noyau (K3 noyau → K4) — hors pile actuelle

Selon `docs/BOOTABLE_KERNEL_ISO_ROADMAP.md` :
1. **K3 noyau** : timer monotone déjà amorcé par K2.9 → stabiliser tâches noyau, changement de contexte et synchronisation ; valider multicœur réel avant d'annoncer SMP (K2.11 fournit l'inventaire).
2. **K4** : ABI système (transitions user/kernel, codes d'erreur), VFS + FS lecture seule, pilote `virtio-blk` testé dans QEMU.
3. **K5** : chargement d'un programme utilisateur, console/entrée, shell minimal.
4. **K6** : ISO hybride UEFI, tests de démarrage à froid/redémarrage, SHA-256 et traces publiés.

Règles inchangées : jamais appeler le service .NET « noyau », aucune simulation présentée comme réelle, chaque jalon avec tests + traces + artefact.

## 5. Étape suivante de l'édition Windows (K3.1, suite du document d'architecture)

Ordre de livraison défini dans `docs/architecture/native-applications-foundation.md` :

1. **K3.1.3 — Fiabilité de l'Explorateur** (priorité haute) : états vide/chargement/erreur, navigation clavier, libellés accessibles, dossiers volumineux, tests (validation de chemin, collisions, cancellation, perte de l'hôte natif). *L'alerte « Native Host indisponible » vue en mode navigateur doit rester un état explicite, jamais un contenu simulé.*
2. **K3.1.4 — Fiabilité des Paramètres** : séparer inventaire Windows réel et préférences locales SYSTEMES ; ne jamais afficher « appliqué » avant confirmation Windows ; confirmation pour les actions perturbatrices.
3. **K3.1.5 — Gestionnaire des tâches** : données mesurées uniquement, intervalles d'échantillonnage affichés, revalidation PID/nom/date avant terminaison.
4. **K3.1.6 — Terminal et Bloc-notes** : PowerShell borné dans l'hôte natif, protection des modifications non enregistrées.
5. **K3.1.7 — Sécurité, sauvegarde, gestion d'applications** : inventaires en lecture seule d'abord ; ne revendiquer antivirus/sauvegarde/Installation que quand l'opération réelle et la voie de récupération sont testées.

## 6. Ce qu'il ne faut pas faire (garde-fous)

- Pas de « mise à jour » de la branche locale par réécriture : `git pull --ff-only` uniquement (fait aujourd'hui).
- Ne pas lancer le moteur natif via Vite pour valider une fonctionnalité Windows : toujours `npm run build` + build Release de l'hôte.
- Ne pas fusionner la pile noyau d'un bloc : elle est empilée, la fusion simultanée casserait les bases.
- Ne pas annoncer SMP/K5/K6 avant les critères de test QEMU correspondants.
