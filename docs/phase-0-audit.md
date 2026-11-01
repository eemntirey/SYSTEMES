# PHASE 0 — AUDIT TECHNIQUE ET PROTECTION

**Référence de travail :** `feat/systemes-native-core` (branche active)  
**Branche de protection :** `main` (protégée, non modifiée)  
**Référence connue :** `feat/settings-windows11-authentique` (commit `f47e1ec`)  
**Dépôt source :** `eemntirey/SYSTEMES` (ce workspace : `C:\Users\eemntirey\Desktop\windowsReproduction`, clone/reproduction active)

---

## 1. ÉTAT RÉEL DU DÉPÔT (vérifié en date du 31/10/2026)

### 1.1 Branchement
- `main` : protégée, aucun commit direct.
- `feat/systemes-native-core` (active) : dérivée de `main` avec historique mixte (`feat/systemes-modernisation-apps`, `fix/audit-functional-ux`).
- `feat/settings-windows11-authentique` (`f47e1ec`) : disponible sur `origin/`. État CI verte selon dernier rapport.
- `feat/systemes-modernisation-apps` : branche parallèle existante.

### 1.2 Code existant (conservé intégralement)
L'interface React 18 + TypeScript + Vite est complète et fonctionnelle. Aucun composant supprimé.

| Couche | Fichier clé | État | Action audit |
|---|---|---|---|
| Layout | `src/App.tsx` | Complet, intègre toutes les apps | **Préserver** |
| État global | `src/context/OSContext.tsx` | 995 lignes, toutes fonctionnalités simulées | **Adapter (pas supprimer)** |
| Types | `src/types/index.ts` | Définis (`AppId`, `WindowState`, `FileItem`, etc.) | **Étendre pour contrats natifs** |
| Constantes | `src/data/constants.ts` | `APP_DEFINITIONS`, `INITIAL_FILES`, `WALLPAPERS` | **Préserver** |
| Desktop | `src/components/desktop/Desktop.tsx`, `DesktopIcon.tsx` | Opérationnel | **Préserver** |
| Taskbar | `src/components/taskbar/Taskbar.tsx` | Opérationnel | **Préserver** |
| StartMenu | `src/components/startmenu/StartMenu.tsx`, `AppTile.tsx` | Opérationnel | **Préserver** |
| Window | `src/components/window/Window.tsx`, `WindowFrame.tsx` | Opérationnel (drag, snap, resize) | **Préserver** |
| Apps intégrées | `FileExplorerApp`, `NotepadApp`, `TerminalApp`, `CalculatorApp`, `SettingsApp`, `PaintApp`, `PhotosApp`, `RecycleBinApp` | Fonctionnelles mais **simulées** | **Transformer progressivement** |
| Styles | `src/styles/index.css` | Néon futuriste (`#8b5cf6`, `#00d4ff`) | **Préserver** |
| Natifs | `native/README.md` | Architecture définie, **aucun code C#/.NET existant** | **Créer (Phase 1)** |

### 1.3 Problèmes détectés (non bloquants pour Phase 0)
- `typecheck-errors.txt` (64 lignes) : erreurs TypeScript dans `SettingsApp.tsx`. Variables manquantes (`addNotification`, `airplaneMode`, `mobileHotspot`, etc.) non liées au contexte `OSContext`. **À corriger avant toute fusion.**
- CI (`.github/workflows/ci.yml`) ne couvre pas `feat/systemes-native-core`. **À ajouter.**
- Aucun test unitaire couvrant `OSContext` natif (normal, moteur non existant).

---

## 2. CARTOGRAPHIE DES FONCTIONNALITÉS SIMULÉES (à transformer en opérations réelles)

| Domaine | Fonction simulée actuelle | Source de données réelle requise | Priorité plan |
|---|---|---|---|
| **Matériel** | Aucune lecture réelle (faux `systeminfo` dans Terminal) | WMI (`Win32_Processor`, `Win32_PhysicalMemory`, `Win32_VideoController`, `Win32_DiskDrive`), capteurs (`MSAcpi_ThermalZoneTemperature`) | Phase 2 |
| **Périphériques** | Liste statique dans `SettingsApp` (`bluetooth`, `network`) | `Win32_PnPEntity`, SetupAPI, `DeviceManager` (USB, BT, Audio, Réseau, Imprimantes) | Phase 3 |
| **Processus / Services** | `tasklist` simulé (`TerminalApp` ligne 303-320) | WMI (`Win32_Process`), `System.Diagnostics.Process`, PerformanceCounter, `Get-Service` (PowerShell contrôlé) | Phase 4 |
| **Windows Update** | Faux message notification (`Windows 11 Pro 24H2 est prêt`) | `WindowsUpdateAgent` (COM `WUA`), API `USOClient`, `Get-WUInstall` | Phase 5 |
| **Paramètres** | Variables locales React (`wifiEnabled`, `bluetoothEnabled`, etc.) sans lien système | `netsh wlan show interface`, `netsh interface show interface`, Registry (`HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion`), PowerShell `Get-NetAdapter`, `DisplayConfig` | Phase 6 |
| **Fichiers** | `INITIAL_FILES` (tableau statique en mémoire) | API `System.IO`, `FileSystemWatcher`, Corbeille Windows (`SHFileOperation`, `IVirtualDesktop`), `RecycleBin` COM | Phase 7 |
| **Applications** | Exploiteur : navigation virtuelle (`createFolder`, `deleteItem` sur array mémoire) | Connexion au disque réel avec validation des permissions (`FileSystemRights`, `AccessCheck`) | Phase 7 |
| **Terminal** | Commandes simulées (`ver`, `systeminfo`, `ping`, `tasklist`) | Exécution contrôlée via `System.Diagnostics.Process` avec `PowerShell` et `cmd.exe` restreints (sandbox) | Phase 7 |
| **Audio / Affichage** | Sliders React sans effet système (`volume`, `brightness`, `resolution`) | `CoreAudio` API, `DisplaySettings`, `PhysicalMonitorBrightnessController` (WMI `WmiMonitorBrightnessMethods`) | Phase 6 |

---

## 3. CONTRATS REACT ↔ MOTEUR NATIF (définis dans `native/README.md`, validés)

Le protocole choisi est **JSON-RPC over named pipes (production Windows) / stdin-stdout (dev)**.
Aucun privilège administrateur permanent n'est accordé au processus React.

### 3.1 Endpoints natifs requis (Phase 1 — Moteur natif)

| Méthode | Direction | Description | Sécurité |
|---|---|---|---|
| `GET /hardware` | React → Native → Windows | CPU, RAM, GPU, Disques, Températures (si capteurs exposés) | Lecture seule, sans élévation |
| `GET /devices` | React → Native | USB (SetupAPI), Bluetooth (`BthLEEnum`), Audio (`MMDeviceEnumerator`), Réseau (`NetAdapter`), Imprimantes (`WmiPrinter`) | Lecture + activation/désactivation autorisée avec confirmation |
| `GET /processes` | React → Native | Liste réelle (`Win32_Process`), CPU %, RAM (PerformanceCounter) | Lecture seule par défaut ; arrêt autorisé (`Kill` + validation) sur demande explicite |
| `GET /services` | React → Native | Services Windows (`Win32_Service`) | Consultation + contrôle (`StartService`/`StopService`) avec autorisation |
| `POST /file/operate` | React → Native | Créer, supprimer, renommer, déplacer (API `System.IO` avec validation chemin et droits) | **Validation stricte des entrées** — pas de chemins arbitraires (`..` bloqué), pas d'exécution |
| `GET /update/check` | React → Native | `WUA` / `USOClient` — mises à jour disponibles | Lecture seule |
| `POST /update/apply` | React → Native | Téléchargement + installation autorisée (`IUpdateDownloader`/`IUpdateInstaller`) avec progress réelle | **Opération sensible** — demande confirmation utilisateur + mécanisme de rollback |
| `POST /settings/apply` | React → Native | Modification paramètres autorisés (`netsh`, Registry, `PowerShell` restreint) | Validation des clés/valeurs, journalisation obligatoire |

### 3.2 Règles de sécurité (non négociables)
1. **Aucun faux résultat simulé présenté comme réel.** Toute réponse doit provenir de l'API Windows ou être explicitement marquée `unavailable`.
2. **Pas de privilèges administrateur permanents.** L'agent natif s'exécute en contexte utilisateur standard ; l'élévation (`UAC`) est demandée uniquement pour les opérations qui l'exigent (`WindowsUpdate` installation, arrêt de processus système, modification réseau globale).
3. **Journalisation obligatoire** (`native/logs/`). Toute opération sensible (`process/kill`, `file/delete`, `update/apply`, `settings/apply`) est loggée avec timestamp, utilisateur, commande et résultat.
4. **Validation des entrées** côté natif. Aucune commande arbitraire n'est exécutée sans whitelist (`allowedCommands`).
5. **Protection contre opérations destructrices.** `WindowsUpdate` et pilotes ne sont jamais modifiés sans mécanisme de récupération (`SystemRestorePoint` ou snapshot).

---

## 4. ARCHITECTURE CIBLE VALIDÉE (Phase 0 → Phase 1)

```
┌─────────────────────────────────────────────────────────────┐
│  REACT (Interface utilisateur — conservée)                  │
│  src/App.tsx, OSContext.tsx, Composants                    │
│  Communication : JSON-RPC (WebSocket local / Named Pipe)    │
└────────────────────┬────────────────────────────────────────┘
                     │ secure IPC
┌────────────────────▼────────────────────────────────────────┐
│  SYSTEMES NATIVE CORE (.NET / C#)                           │
│  native/core/SystemesNativeAgent.exe                       │
│  Modules : Hardware, Device, Process, FileSystem,          │
│  Network, Display, Audio, WindowsUpdate, SettingsAgent      │
│  Communication : Named Pipes (prod) / stdin-stdout (dev)     │
│  Sécurité : Sandbox, Validation, Logging, No permanent admin│
└────────────────────┬────────────────────────────────────────┘
                     │ Windows APIs (WMI, Win32, WinRT, PS)
┌────────────────────▼────────────────────────────────────────┐
│  MICROSOFT WINDOWS 11                                       │
│  WMI | SetupAPI | PerformanceCounter | CoreAudio | Netsh     │
│  Registry | PowerShell (restreint) | UAC (élévation ciblée)│
└──────────────────────────────────────────────────────────────┘
```

---

## 5. CONTRATS ENTRE REACT ET MOTEUR NATIF (définis pour Phase 1)

### 5.1 Format de message (JSON-RPC 2.0 simplifié)
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "method": "hardware/read",
  "params": {},
  "auth": "session-token-local"
}
```

Réponse :
```json
{
  "jsonrpc": "2.0",
  "id": "req-001",
  "result": {
    "cpu": { "name": "AMD Ryzen 9 7950X", "cores": 16, "usagePercent": 12 },
    "ram": { "totalGB": 32, "usedGB": 14.3, "availableGB": 17.7 },
    "gpu": { "name": "NVIDIA GeForce RTX 4090", "memoryGB": 24 },
    "disks": [{ "name": "NVMe SSD (C:)", "totalBytes": 512107374182, "freeBytes": 152550369792 }],
    "status": "real",
    "source": "WMI"
  },
  "error": null
}
```

### 5.2 État d'indisponibilité (obligatoire si Windows ne répond pas)
```json
{
  "jsonrpc": "2.0",
  "id": "req-002",
  "result": null,
  "error": {
    "code": -32601,
    "message": "Hardware sensor unavailable: no WMI thermal zone driver loaded",
    "data": { "available": false, "reason": "no_thermal_zone_driver" }
  }
}
```

---

## 6. PROTECTION DES BRANCHES ET LIVRABLES

### 6.1 Règles Git
- `main` reste protégée. Aucune fusion directe.
- `feat/systemes-native-core` reçoit les commits distincts et vérifiables.
- Chaque phase est un commit séparé (ex: `feat(native): init .NET agent`, `feat(native): hardware WMI module`, etc.).
- Avant toute fusion dans `main`, CI doit être entièrement verte (`npm run typecheck`, `npm test`, `npm run build`).

### 6.2 Livrable Phase 0 (ce document)
- [x] Audit du dépôt complet (fichiers lus : `package.json`, `App.tsx`, `OSContext.tsx`, `types/index.ts`, `constants.ts`, `SettingsApp.tsx`, `FileExplorerApp.tsx`, `TerminalApp.tsx`, `Desktop.tsx`, `Window.tsx`, `Taskbar.tsx`, `native/README.md`, `.github/workflows/ci.yml`)
- [x] Identification des fonctionnalités simulées (voir tableau section 2)
- [x] Identification des composants React à conserver (voir tableau section 1.2)
- [x] Vérification des branches (`main`, `feat/systemes-native-core`, `feat/settings-windows11-authentique`, `feat/systemes-modernisation-apps`)
- [x] Vérification des tests et CI (`.github/workflows/ci.yml` ne couvre pas la branche active)
- [x] Définition des contrats React ↔ Moteur natif (section 5)

---

## 7. RECOMMANDATION IMMÉDIATE (avant modification de code)

1. **Corriger `typecheck-errors.txt`** dans `src/components/apps/SettingsApp.tsx` afin que la CI soit verte sur la branche active. C'est un prérequis non négociable du plan (règle 8 : « Toute modification doit être accompagnée de tests », et règle 10 : « CI entièrement verte avant fusion »).
2. **Créer le projet C#/.NET** dans `native/core/` avec la structure module par module définie dans `native/README.md`.
3. **Mettre en place la première communication** (`stdin-stdout` en mode dev) entre React et le processus natif — même avec une réponse vide — pour prouver que le canal IPC fonctionne.
4. **Ajouter `feat/systemes-native-core`** au fichier `.github/workflows/ci.yml`.

---

## 8. DÉCISION TECHNIQUE CONFIRMÉE

- **React + TypeScript** : conservé (pas de réécriture).
- **Vite** : conservé.
- **Moteur natif** : C# / .NET 8 (ou .NET Framework 4.8 si compatibilité Windows 11 22H2 requise sans runtime .NET 8). Compte tenu des APIs Windows (WMI, SetupAPI, WinRT, PowerShell), **.NET 8 avec `System.Management` et P/Invoke** est le choix le plus robuste.
- **WebView2** : non requis pour la Phase 1 ; l'interface React reste dans le navigateur (ou dans une app Electron/WebView2 plus tard, Phase 1 optionnelle).
- **Agent natif** : exécutable console (`SystemesNativeAgent.exe`) lancé par le processus React (ou comme service local si nécessaire en Phase 5+).

---

*Audit réalisé le 31/10/2026 sur la branche `feat/systemes-native-core`.*
*Aucune suppression de code existant. Aucune réécriture d'interface. Aucun faux résultat généré.*
