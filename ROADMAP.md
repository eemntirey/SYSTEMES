# SYSTEMES — ROADMAP fonctionnelle vérifiée

Référence audit : `main` à `0b732fd64e139ce45d59eca8f09d304b82179f10` (2026-11-05).
Le statut décrit le code présent dans ce commit, pas une promesse produit ni une validation matérielle.
- **Fait** : implémentation et preuve automatisée visibles pour le périmètre indiqué.
- **Partiel** : socle ou simulation présent, couverture incomplète.
- **Absent** : aucun composant opérationnel identifié.
- Une fonctionnalité « simulée » n'est jamais considérée comme une opération Windows réelle ou comme une fonction du noyau autonome.

## 1. Interface et expérience de bureau — PARTIEL
- [x] Bureau, icônes et fond d'écran — `src/components/desktop/Desktop.tsx`, `DesktopIcon.tsx`, `src/data/constants.ts`.
- [x] Barre des tâches et menu Démarrer — `src/components/taskbar/`, `src/components/startmenu/`.
- [x] Fenêtres, déplacement, redimensionnement et Snap de base — `src/components/window/WindowFrame.tsx`.
- [ ] Écran de verrouillage/OOBE complet, thèmes et contraste de bout en bout — `src/components/lockscreen/LockScreen.tsx`, `src/styles/`.

## 2. Multitâche et bureaux virtuels — PARTIEL
- [x] Fenêtres, focus et Snap de base — `src/components/window/`, `src/context/OSContext.tsx`.
- [ ] Snap Groups/Assist complet, multi-écrans natifs, bureaux persistants, Alt+Tab et Shake — `src/components/taskview/TaskView.tsx`, `src/context/OSContext.tsx`.

## 3. Paramètres Windows — PARTIEL
- [x] Application Paramètres et ouverture de pages Windows via hôte natif — `src/components/apps/SettingsApp.tsx`, `src/services/NativeBridge.ts`, `native/Windver/Core/SettingsNative.cs`.
- [ ] Inventaire exhaustif des 11 catégories, application confirmée de chaque réglage et tests par réglage — mêmes chemins.

## 4. Explorateur de fichiers — PARTIEL
- [x] UI Explorateur et opérations natives de base (liste, lecture, création, écriture, déplacement, copie, renommage, suppression) — `src/components/apps/FileExplorerApp.tsx`, `src/utils/fileSafety.ts`, `src/utils/useSafeFileOps.ts`, `native/Windver/Core/FileSystemNative.cs`.
- [ ] Onglets, Accueil/Galerie, volet détails, archives, indexation, corbeille système complète et synchronisation cloud — mêmes chemins.

## 5. Applications intégrées — PARTIEL
- [x] Explorateur, Bloc-notes, Terminal, Calculatrice, Paint, Photos, Gestionnaire des tâches, Corbeille et Horloge (niveaux variables) — `src/components/apps/`.
- [ ] Suite complète et parité fonctionnelle, persistance et tests E2E systématiques — `src/components/apps/`.

## 6. Store, catalogue et gestion de paquets — PARTIEL
- [x] Entrée Store présente — `src/components/apps/StoreApp.tsx`.
- [ ] Catalogue, installation/désinstallation vérifiées, mises à jour et gestionnaire de paquets compatible — `src/components/apps/StoreApp.tsx`, moteur natif.

## 7. IA et assistance — ABSENT
- [ ] Assistant configurable, recherche sémantique, actions contextuelles, dictée et Recall opt-in — aucun module produit dédié identifié dans `src/`, `native/` ou `native-core/`.

## 8. Sécurité — PARTIEL
- [x] Lecture du contexte de sécurité Windows, permissions et inventaire de capacités — `native/Windver/Core/SecurityManager.cs`, `native/Windver/Core/NativeCapabilities.cs`, `src/services/NativeLifecycle.ts`.
- [ ] Antivirus/quarantaine, SmartScreen, VBS/HVCI, TPM, Secure Boot, Sandbox et chiffrement opérationnels — pas d'implémentation complète identifiée.

## 9. Confidentialité — ABSENT
- [ ] Permissions caméra/micro/localisation par application, indicateurs et tableau de diagnostics — aucun centre de confidentialité complet identifié.

## 10. Réseau et connectivité — PARTIEL
- [x] Inventaire réseau, état/scan/connexion Wi-Fi et état Bluetooth via méthodes natives — `native/Windver/Core/NetworkManager.cs`, `RadioControlNative.cs`, `src/services/NativeBridge.ts`.
- [ ] VPN, DoH, pare-feu, partage de proximité, RDP, diagnostics avancés et tests matériels multi-adaptateurs — couverture non établie.

## 11. Stockage — PARTIEL
- [x] Volumes et opérations de fichiers — `native/Windver/Core/StorageManager.cs`, `FileSystemNative.cs`.
- [ ] Storage Sense, espaces de stockage, sauvegarde, Historique des fichiers, restauration et Dev Drive — absents ou non validés.

## 12. Performances et ressources — PARTIEL
- [x] Mesures matérielles/mémoire/processus et vues UI — `native/Windver/Core/HardwareManager.cs`, `MemoryManager.cs`, `ProcessManager.cs`, `src/components/apps/ProcessManagerApp.tsx`.
- [ ] Historique fiable, compteurs détaillés disque/réseau/GPU, profilage et objectifs de performance — incomplets.

## 13. Jeu et multimédia — ABSENT
- [ ] Mode Jeu, Game Bar, audio spatial, capture et optimisation GPU — aucun ensemble fonctionnel complet identifié.

## 14. Développement et outils avancés — PARTIEL
- [x] Terminal natif via hôte Windows — `native/Windver/Core/TerminalNative.cs`, `src/components/apps/TerminalApp.tsx`.
- [ ] Profils multiples, WSL-like, gestionnaire de paquets, PowerToys-like et Dev Drive — absents ou incomplets.

## 15. Virtualisation — ABSENT
- [ ] Hyper-V, WSL2, Windows Sandbox et gestion de machines virtuelles — aucune implémentation identifiée.

## 16. Accessibilité — PARTIEL
- [x] Structure d'interface et composants interactifs existants — `src/components/`, `src/styles/`.
- [ ] Narrateur, loupe, filtres, sous-titres, contrôle vocal/oculaire et audit WCAG complet — non implémentés ou non vérifiés.

## 17. Administration et gestion d'entreprise — ABSENT
- [ ] GPO/MDM, éditions, kiosque/Assigned Access et Autopilot — aucune implémentation identifiée.

## 18. Mises à jour — PARTIEL
- [x] Lecture/contrôle limité du statut Windows Update — `native/Windver/Core/WindowsUpdateNative.cs`, `src/services/NativeBridge.ts`.
- [ ] Orchestration complète, pause, planification, rollback et récupération vérifiés — non implémentés.

## 19. Dépannage et récupération — ABSENT
- [ ] WinRE, points de restauration, réparation et réinitialisation du système — aucune implémentation opérationnelle identifiée.

## 20. Raccourcis clavier et navigation — PARTIEL
- [x] Gestionnaires de clavier et raccourcis ponctuels dans les composants — `src/components/`, `src/context/OSContext.tsx`.
- [ ] Table exhaustive des raccourcis du guide, couverture et tests automatisés — non démontrés.

## 21. Comptes, identité et sessions — PARTIEL
- [x] Lecture du jeton Windows et des permissions courantes — `native/Windver/Core/SecurityManager.cs`.
- [ ] Comptes SYSTEMES, groupes, RBAC complet, UAC/Hello/passkeys et sessions — absents ou simulés.

## 22. Notifications, calendrier et widgets — PARTIEL
- [x] Centre d'action, réglages rapides, calendrier et widgets — `src/components/actioncenter/`, `src/components/widgets/`.
- [ ] Notifications persistantes, règles DND, synchronisation et sessions Focus — incomplets.

## 23. Internationalisation et langues — ABSENT
- [ ] FR/EN/MG avec formats régionaux et tests — aucun système i18n complet identifié.

## 24. Expérience tactile, stylet et mobilité — ABSENT
- [ ] Gestes tactiles, stylet, Phone Link/mobile et continuité — aucune implémentation complète identifiée.

## 25. Plateforme native Windows et intégration système — PARTIEL
- [x] Hôte WPF + WebView2 et moteur .NET séparé — `native/windows-host/`, `native/Windver/Core/`.
- [x] Transport WebView2, protocole JSON sur stdin/stdout, façade TypeScript et contrôles d'accès — `src/native/NativeBridge.ts`, `src/services/NativeBridge.ts`, `native/windows-host/NativeProcessHost.cs`, `native/Windver/Core/NativeServer.cs`.
- [ ] Couverture exhaustive des API Windows, packaging installable et tests de compatibilité — incomplets.

## 26. Noyau autonome Rust, démarrage et livraison — PARTIEL
- [x] Contrat BootInfo, noyau `no_std`, chargeur UEFI, GOP/ACPI discovery, GDT/IDT, exceptions, allocateur physique et pagination de base — `kernel/src/`, `boot/uefi-loader/main.c`.
- [x] Pipeline QEMU/OVMF défini — `.github/workflows/kernel.yml`.
- [ ] Ordonnanceur préemptif, SMP, syscalls, mode utilisateur, pilotes complets, VFS, shell et ISO avec interface utilisateur complète — absents.
- [ ] Livraison v1.0, démo et guide utilisateur complet — absents.

---

## État des validations au commit audité

| Validation | État | Preuve |
|---|---|---|
| CI principale sur le HEAD de `main` | **Réussie** | [run 41993456370](https://github.com/eemntirey/SYSTEMES/actions/runs/41993456370) |
| Frontend typecheck/tests/build | Inclus dans la CI réussie | `.github/workflows/ci.yml` |
| Hôte WPF + moteur .NET + smoke tests | Inclus dans la CI réussie | `.github/workflows/ci.yml` |
| Build Rust/QEMU sur le HEAD exact de `main` | **Non établi** | Workflow noyau déclenché sur PR et push `Osreell`, pas sur chaque push `main` |
| Build local reproductible dans l'environnement d'audit | **Non exécuté** | Clone HTTPS bloqué par résolution DNS de `github.com` |

## Priorités d'exécution

1. Établir une CI Rust obligatoire sur les PR et sur la branche de développement noyau ; vérifier chaque HEAD, sans réutiliser un ancien résultat.
2. Terminer et valider les primitives noyau dans l'ordre : K2 mémoire/protections, K2.7 ordonnanceur, interruptions/timer, SMP, syscalls et mode utilisateur.
3. Consolider les applications Windows une par une : Explorateur, Paramètres, Gestionnaire des tâches, Terminal, Bloc-notes.
4. Compléter les fonctionnalités système et la sécurité sans présenter une simulation comme une opération réelle.
5. Ajouter E2E Playwright, tests d'intégration, audit dépendances et packaging.
6. Ne publier v1.0 qu'après ISO bootable, shell utilisateur, preuves QEMU, documentation et CI entièrement verte.
