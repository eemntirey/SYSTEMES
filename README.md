# SYSTEMES — Recréation de Windows 11

## 🎯 Mission du projet

**Ce projet est développé uniquement à des fins éducatives, de recherche et d'utilisation personnelle gratuite.**

- ✅ **Gratuit** — Aucun coût, aucun abonnement, aucune limitation d'usage
- ✅ **Sans licence restrictive** — Code source ouvert, utilisation libre sans contrainte juridique
- ✅ **Non commercial** — Aucune monétisation, aucune vente, aucune collecte de données
- ✅ **Open Source** — Développement transparent, contributions bienvenues

> **Déclaration formelle** : SYSTEMES n'a **aucun but lucratif**. Il ne sera jamais vendu, licencié, ni monétisé sous quelque forme que ce soit. Toute utilisation commerciale dérivée est explicitement interdite par la volonté des auteurs.

## 📋 Description

Recréation fidèle de l'interface Windows 11 en **React 18 + Vite + TypeScript**, avec un **moteur natif .NET 8** qui interagit réellement avec le système Windows (WMI, Win32, APIs système) — **aucune simulation**.

### Fonctionnalités principales
- Bureau, barre des tâches, menu Démarrer (thème Obsidian glassmorphism)
- Explorateur de fichiers natif (lecture/écriture réelle)
- Gestionnaire de processus réel (listing, terminaison, métriques)
- Panneau matériel : CPU, RAM, GPU, disques, réseau, batterie
- Terminal natif, Calculatrice, Bloc-notes, Paint, Photos
- Paramètres Windows réels (ms-settings:), Windows Update, services
- Notifications natives, centre d'actions, widgets

## 🏗 Architecture

```
├── src/                    # Application React (UI)
│   ├── components/         # Composants UI (Desktop, Taskbar, StartMenu, Windows, Apps)
│   ├── native/             # Pont vers le moteur .NET (NativeBridge, JSON-RPC)
│   └── styles/             # CSS (dont obsidian-desktop.css pour le thème glass)
├── native/
│   ├── Windver/Core/       # Moteur .NET 8 (C#) — HardwareManager, ProcessManager, FileSystemNative, etc.
│   ├── windows-host/       # Hôte WPF + WebView2 (build natif autonome)
│   └── native-core/        # Contrats partagés (ISystemServices)
├── dist/                   # Build de production (servi par l'hôte natif)
└── docs/                   # Documentation technique
```

## 🚀 Lancement rapide

### Version navigateur (développement)
```bash
npm install
npm run dev          # http://127.0.0.1:5173
```

### Version native Windows (recommandée — sans navigateur)
```bash
npm run build
dotnet build native/windows-host/SYSTEMES.WindowsHost.csproj -c Release
.\native\windows-host\bin\Release\net8.0-windows\SYSTEMES.WindowsHost.exe
```

Prérequis : **.NET 8 SDK**, **Node.js 20+**, **WebView2 Runtime** (inclus dans Windows 11).

## 🤝 Contribution

Contributions bienvenues (issues, PR, idées) — dans le respect de la mission **gratuite et non-commerciale**.

## 📜 Avertissement légal

- Ce projet n'est **pas affilié à Microsoft**.
- "Windows 11" est une marque déposée de Microsoft Corporation.
- SYSTEMES est une **re-création indépendante** à but éducatif.
- Le code natif utilise des APIs Windows documentées (WMI, Win32, .NET) — aucun exploit, aucune élévation de privilège non consentie.

---

**Fait avec passion par la communauté, pour la communauté. 💙**