# Windows 11 Recreation — Design Spec

## Overview
Reproduction interactive du desktop Windows 11 avec un style **inspiré/custom** (mode sombre néon / futuriste), en React + Vite + TypeScript.

## Scope
- **Phase 1** : Desktop interactif + Barre des tâches + Menu Démarrer
- **Phase 2** : Fenêtres flottantes décoratives (Explorateur, Bloc-notes, Paramètres)
- **Style** : Dark néon / futuriste (couleurs violet/néon bleu), coins très arrondis, sans effet glass obligatoire.
- **Non inclus** : logique réelle de système de fichiers, édition réelle, persistance, multi-utilisateur.

## Architecture
```
App (layout plein écran)
  └── Desktop (fond + grille d'icônes + clic droit)
  └── Taskbar (icônes centrées, bouton Démarrer, heure, notifications)
  └── StartMenu (panneau flottant avec apps épinglées + liste)
  └── Window (composant réutilisable de fenêtre flottante)
```

## Components
| Component | Responsibility | Key Props/State |
|---|---|---|
| `Desktop` | Fond d'écran, grille d'icônes, clic droit contextuel | `icons[]`, `onIconClick` |
| `DesktopIcon` | Icône individuelle (image + label) | `label`, `iconUrl`, `position` |
| `Taskbar` | Barre inférieure centrée, icônes, bouton Démarrer, heure | `apps[]`, `onAppClick`, `startOpen` |
| `StartMenu` | Panneau avec grille d'apps épinglées + liste | `pinnedApps[]`, `allApps[]`, `onAppLaunch` |
| `Window` | Fenêtre décorative flottante (drag possible, boutons fermer/minimiser) | `title`, `children`, `isOpen`, `position` |

## Data Flow
- `App` gère l'état global : `startMenuOpen`, `windows[]` (fenêtres ouvertes), `desktopIcons[]`.
- `Desktop` reçoit `icons` et `onIconClick`.
- `Taskbar` reçoit `apps` et `onAppClick` ; le clic sur Démarrer bascule `startMenuOpen` via `App`.
- `Window` est contrôlée par `windows` dans `App`.

## Interactions
- **Clic icône desktop** : ouvre `Window` correspondante.
- **Clic bouton Démarrer** : ouvre/ferme `StartMenu`.
- **Clic app dans StartMenu** : ouvre `Window`.
- **Fenêtre** : drag (déplacement) décoratif, bouton fermer (retire de `windows`), bouton minimiser (réduit visuellement).

## Visual Design (Custom — Dark Néon / Futuriste)
- **Palette** : fond `#0a0a12` (presque noir), accents `#8b5cf6` (violet néon) et `#00d4ff` (cyan néon), texte blanc/gris clair.
- **Formes** : coins très arrondis (`border-radius: 20px` min, `24px` sur fenêtres), ombres profondes avec léger glow néon.
- **Taskbar** : fond `rgba(10, 10, 18, 0.7)` avec `backdrop-filter: blur(12px)`, icônes centrées avec glow au hover.
- **Menu Démarrer** : même fond flou, grille d'apps en cards arrondies avec hover glow.
- **Fenêtres** : barre de titre sombre, boutons de contrôle ronds, corps avec léger glow de bordure.

## Testing
- Tests unitaires sur `Desktop`, `Taskbar`, `Window` avec `@testing-library/react`.
- Vérification visuelle manuelle : ouvertures, fermetures, drag, hover.

## Implementation Plan (high-level)
1. Créer `src/components/` et `src/App.tsx`.
2. Phase 1 : `Desktop`, `Taskbar`, `StartMenu`, `DesktopIcon`.
3. Phase 2 : `Window` réutilisable avec drag décoratif.
4. Intégration dans `App` avec gestion d'état.
5. Styles CSS/SCSS ou Tailwind (si ajouté au projet) pour le thème néon.
