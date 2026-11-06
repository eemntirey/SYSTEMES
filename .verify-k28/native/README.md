# SYSTEMES Native Core — Phase 1 Architecture

## Principe fondamental
Le moteur natif agit sur le véritable système Windows 11. Il ne simule pas.

## Communication React ↔ .NET
- Protocole : JSON-RPC over named pipes (Windows) / stdin-stdout (dev)
- Sécurité : pas de privilèges administrateur permanents
- Journalisation : toutes les opérations sensibles loggées

## Modules natifs
- HardwareManager (WMI / Win32)
- DeviceManager (SetupAPI / Win32)
- ProcessManager (PerformanceCounter / WMI)
- FileSystem (Windows API)
- NetworkManager (Wlan API / Netsh)
- DisplayManager (DisplaySettings)
- AudioManager (CoreAudio)
- WindowsUpdate (WUA / USO)

## Contrats (API vers React)
- `GET /hardware` → CPU, RAM, GPU, Disques
- `GET /devices` → USB, Bluetooth, Audio, Réseau
- `GET /processes` → Liste, CPU, RAM
- `POST /file/operate` → Créer, supprimer, renommer (avec validation)
- `POST /update/check` → Vérifier mises à jour
- `POST /settings/apply` → Modifier paramètres autorisés

Aucune opération ne doit prétendre réussir sans confirmation Windows.
