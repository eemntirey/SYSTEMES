# SYSTEMES — Architecture technique

Date de référence : 2026-11-05  
Commit audité : `0b732fd64e139ce45d59eca8f09d304b82179f10`

## 1. Objectif et frontières

SYSTEMES est un projet indépendant, gratuit et original, inspiré des usages d'un système de bureau moderne. Il ne doit pas incorporer d'assets propriétaires Microsoft (icônes, fonds, polices ou sons). Les ressources doivent être libres, originales ou générées, avec leurs licences documentées.

Le dépôt contient deux produits qui doivent rester découplés :

1. **Édition Windows hôte** : shell React hébergé dans WPF/WebView2, moteur .NET qui effectue les opérations réelles autorisées sur Windows.
2. **Noyau autonome** : noyau Rust `no_std` démarré par UEFI et exécuté sans dépendre du Windows hôte.

L'édition Windows n'est pas le noyau. Le moteur .NET n'est pas un kernel. Le noyau Rust ne doit pas dépendre de React, WPF, WebView2 ou des API Windows.

## 2. Vue d'ensemble

```text
ÉDITION WINDOWS
┌──────────────────────────────────────────────┐
│ React 18 + TypeScript + Vite                 │
│ src/components, src/context, src/services    │
└─────────────────────┬────────────────────────┘
                      │ WebView2 postMessage / WebMessageReceived
┌─────────────────────▼────────────────────────┐
│ Hôte Windows WPF (.NET 8)                   │
│ native/windows-host/                         │
│ - cycle de vie WebView2                      │
│ - validation et routage des messages         │
│ - propriétaire du processus natif           │
└─────────────────────┬────────────────────────┘
                      │ JSON ligne par ligne, stdin/stdout UTF-8
┌─────────────────────▼────────────────────────┐
│ SYSTEMES Native Engine (.NET 8)              │
│ native/Windver/Core/                         │
│ - NativeServer / dispatcher                  │
│ - Hardware, Process, Files, Network, etc.    │
└─────────────────────┬────────────────────────┘
                      │ WMI, Win32, .NET, URI ms-settings, etc.
┌─────────────────────▼────────────────────────┐
│ Windows 10/11 — permissions du compte       │
└──────────────────────────────────────────────┘

NOYAU AUTONOME (CIBLE SÉPARÉE)
┌──────────────────────────────────────────────┐
│ Firmware UEFI / GOP / ACPI                  │
└─────────────────────┬────────────────────────┘
                      │ GNU-EFI loader / ExitBootServices
┌─────────────────────▼────────────────────────┐
│ boot/uefi-loader                             │
│ charge KERNEL.BIN et passe BootInfo          │
└─────────────────────┬────────────────────────┘
                      │ x86_64 System V ABI
┌─────────────────────▼────────────────────────┐
│ kernel/ — Rust no_std, x86_64-unknown-none  │
│ GDT/IDT, exceptions, physical frames, paging │
└──────────────────────────────────────────────┘
```

## 3. Frontend

- Entrée : `src/main.tsx`, `src/App.tsx`.
- Shell : `src/components/desktop/`, `taskbar/`, `startmenu/`, `window/`, `taskview/`.
- Applications : `src/components/apps/`.
- État de session et du bureau : `src/context/OSContext.tsx`.
- Contrats : `src/types/`.
- Transport natif browser-safe : `src/native/NativeBridge.ts`.
- Façade applicative typée : `src/services/NativeBridge.ts`.
- Cycle de vie natif partagé : `src/services/NativeLifecycle.ts`.

Le rendu React ne lance aucun processus Windows et n'accède pas directement aux API système. Les composants doivent appeler la façade typée ; les méthodes IPC brutes restent encapsulées dans le transport.

## 4. Contrat React ↔ hôte WPF

Le canal est le transport de messages WebView2 :
- React envoie un objet contenant `id`, `method` et `params` via `window.chrome.webview.postMessage`.
- WPF reçoit via `CoreWebView2.WebMessageReceived`.
- L'hôte vérifie et transmet la requête au processus natif.
- La réponse JSON est renvoyée dans WebView2 avec le même identifiant.

Fichiers de référence :
- `src/native/NativeBridge.ts`
- `native/windows-host/MainWindow.xaml.cs`
- `native/windows-host/NativeProcessHost.cs`

Le navigateur classique n'est pas un hôte natif pris en charge ; il doit afficher un état `unavailable`, jamais inventer des données matérielles.

## 5. Contrat hôte WPF ↔ moteur .NET

L'hôte WPF est propriétaire du cycle de vie de `SystemesNative.exe` :
- démarrage contrôlé au chargement de la fenêtre ;
- handshake `system/ping` avant de considérer le moteur prêt ;
- stdin/stdout redirigés en UTF-8 ;
- une requête JSON par ligne et une réponse JSON par ligne ;
- stderr drainé séparément ;
- sérialisation des échanges pour éviter l'entrelacement des réponses ;
- arrêt gracieux avec délai, puis terminaison de l'arbre de processus si nécessaire.

Références :
- `native/windows-host/NativeProcessHost.cs`
- `native/Windver/Core/NativeServer.cs`
- `native/Windver/Program.cs`
- `native/Windver/Core/SystemesNative.csproj`

La version de protocole est annoncée par `system/ping`. Les méthodes doivent être versionnées ou évoluer de manière rétrocompatible ; toute modification de contrat exige des tests côté client et serveur.

## 6. Sécurité et résilience

1. Liste blanche des méthodes IPC côté frontend et dispatch explicite côté moteur.
2. Paramètres JSON de type objet, taille bornée, identifiants de requête corrélés.
3. Délais d'expiration et limite de requêtes en attente.
4. Erreurs structurées : `permission-required`, `unsupported`, `host-unavailable`, `timeout`, `not-found`, `invalid-argument`, `io-error`, `native-error`.
5. Aucune élévation permanente ; exécution avec le jeton Windows courant.
6. Confirmation utilisateur proportionnelle à l'impact pour les mutations sensibles.
7. Les échecs, permissions refusées et capacités absentes restent visibles ; aucune donnée factice ne doit les remplacer.
8. Pas de PowerShell arbitraire provenant du renderer ; les commandes terminal sont exécutées dans le moteur avec limites et validations.

Le cycle de vie d'une application native distingue `checking`, `ready`, `unavailable`, `unsupported`, `permission-required` et `error`.

## 7. Moteur .NET — domaines présents

Code principal : `native/Windver/Core/`.
Les domaines présents couvrent notamment matériel, processus, fichiers, périphériques, réseau, audio, affichage, alimentation, sécurité, services, stockage, terminal et Windows Update. La présence d'un fichier ne prouve pas que toute la fonctionnalité est achevée : les capacités, erreurs et limites doivent être exposées explicitement et testées.

Le projet hôte `native/windows-host/SYSTEMES.WindowsHost.csproj` publie le moteur natif sous `native-engine/`. La CI construit le projet WPF et exécute un handshake ainsi que des opérations natives temporaires.

## 8. Noyau Rust — frontière actuelle

- Crate : `kernel/Cargo.toml`, `kernel/src/`.
- Cible : `x86_64-unknown-none`, `no_std`, binaire sans runtime standard.
- Entrée : `_start(BootInfo*)`.
- Contrat versionné : `BootInfo` défini dans `kernel/src/lib.rs`.
- Chargeur : `boot/uefi-loader/main.c`, GNU-EFI.
- Chargeur : réserve l'image noyau à l'adresse physique 2 MiB, découvre GOP et ACPI RSDP, récupère la carte mémoire et appelle `ExitBootServices`.
- Noyau : initialise une GDT/IDT, les exceptions CPU, l'allocateur de frames physiques et une pagination identité de base.

Limites connues à cette révision :
- chargement noyau à adresse fixe et image plate, sans relocation ELF ;
- mapping identité bas de 512 GiB, sans séparation complète noyau/utilisateur ;
- pas encore de heap général, ordonnanceur complet, SMP, syscalls, mode utilisateur, VFS, pilotes complets ni shell ;
- GOP fournit un affichage de démarrage élémentaire, pas un compositeur graphique ;
- pas de compatibilité matérielle générale revendiquée.

Le noyau ne doit utiliser que des pilotes virtuels testés QEMU avant d'étendre le support au matériel physique.

## 9. CI et qualité

`.github/workflows/ci.yml` :
- Node 20, `npm ci`, typecheck, Vitest et build Vite ;
- Windows runner, .NET 8, build WPF/WebView2 et moteur ;
- handshake JSON et tests d'intégration sur fichiers temporaires et services.

`.github/workflows/kernel.yml` :
- tests Rust du contrat ;
- `cargo check` pour `x86_64-unknown-none` ;
- compilation GNU-EFI, création ISO et démarrage QEMU/OVMF avec vérification des marqueurs série/debug.

À corriger dans la politique CI : exiger les contrôles noyau sur chaque PR touchant le noyau et protéger la branche d'intégration. Un succès CI sur un autre SHA n'est jamais une preuve pour le HEAD courant.

## 10. Décisions de livraison

- Un incrément fonctionnel par branche et commit conventionnel.
- Tests ciblés avant toute modification risquée.
- Build et tests verts sur le HEAD exact avant fusion.
- Pas de fusion forcée ni de contournement des protections.
- Pas d'outil Kilo Code requis.
- Aucun changement sans rapport avec le jalon.
- Maintenir `ROADMAP.md` et `CHANGELOG.md`.
- Release 1.0 seulement après ISO bootable, shell utilisateur fonctionnel, CI complète, packaging et documentation.
