# Windows 11 Recreation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Recréer le desktop Windows 11 interactif (Phase 1 + Phase 2) avec style dark néon futuriste, sur la base React + TypeScript existante.

**Architecture:** Utiliser le `OSContext` existant pour la gestion d'état globale. `Desktop` affiche le fond et les icônes. `Taskbar` affiche la barre des tâches. `StartMenu` est le panneau d'applications. `WindowFrame` gère les fenêtres flottantes. `Window` intègre le contenu des applications.

**Tech Stack:** React 19, TypeScript, Vite, CSS Variables, Context API.

**Spec:** docs/superpowers/specs/2026-10-31-windows11-recreation-design.md

---

## Global Constraints
- Style visuel : dark néon futuriste (violet `#8b5cf6`, cyan `#00d4ff`, fond `#0a0a12`, coins arrondis `20-24px`, glow néon).
- Pas de logique réelle de système de fichiers au-delà de ce qui existe déjà (`FileItem`, `createFile`, etc. sont déjà fonctionnels).
- Composants séparés avec interfaces claires, tests avec `@testing-library/react`.
- Ne pas supprimer le code existant mais l'adapter au style demandé.

---

## File Structure
```
src/
  App.tsx                    # Layout principal avec Desktop, Taskbar, StartMenu, Windows
  context/OSContext.tsx     # Déjà complet (gestion fenêtres, flyouts, fichiers, notifications)
  types/index.ts             # Déjà complet
  data/constants.ts         # Déjà complet
  styles/index.css          # Adapter au thème néon futuriste
  components/
    common/FluentIcon.tsx    # Déjà présent
    desktop/Desktop.tsx      # Nouveau / adapter : fond d'écran, icônes de bureau
    desktop/DesktopIcon.tsx  # Nouveau : icône individuelle sur le bureau
    taskbar/Taskbar.tsx      # Adapter : barre des tâches centrée, style néon
    taskbar/TaskbarIcon.tsx  # Adapter : icône d'application dans la barre
    startmenu/StartMenu.tsx  # Adapter : panneau applications, style néon
    startmenu/AppTile.tsx    # Adapter : tuile d'application dans le menu
    window/WindowFrame.tsx   # Déjà présent : fenêtre décorative complète
    window/Window.tsx        # Adapter : intégration des apps dans les fenêtres
```

---

## Task List

### Task 1: Adapt Visual Style (CSS Variables) — Dark Neon Futuriste
**Files:** Modify `src/styles/index.css`

- [x] Update CSS variables to dark neon palette
Modifier `:root` dans `src/styles/index.css` : `--win-bg`: `#0a0a12`, `--win-accent`: `#8b5cf6`, `--win-accent-hover`: `#a78bfa`, `--win-accent-active`: `#7c3aed`, `--win-accent-light`: `rgba(139,92,246,0.18)`, `--taskbar-bg`: `rgba(10,10,18,0.75)`, `--taskbar-border`: `rgba(139,92,246,0.15)`, `--taskbar-item-hover`: `rgba(139,92,246,0.12)`, `--taskbar-item-active`: `rgba(139,92,246,0.25)`, `--flyout-bg`: `rgba(10,10,18,0.85)`, `--flyout-border`: `rgba(139,92,246,0.2)`, `--flyout-card`: `rgba(139,92,246,0.06)`, `--flyout-card-hover`: `rgba(139,92,246,0.15)`, `--window-bg`: `rgba(10,10,18,0.92)`, `--window-titlebar`: `rgba(10,10,18,0.9)`, `--window-border`: `rgba(139,92,246,0.3)`, `--text-primary`: `#f0f0f5`, `--text-secondary`: `#a0a0c0`, `--radius-sm`: `12px`, `--radius-md`: `16px`, `--radius-lg`: `20px`, `--radius-xl`: `24px`.

- [x] Add glow animations and neon effects
Ajouter `.neon-glow` et `.text-glow` dans `src/styles/index.css`.

- [x] Verify style changes
Ouvrir `npm run dev`, vérifier le rendu visuel.

---

### Task 2: Desktop Component — Interactive Background + Icons
**Files:** Create `src/components/desktop/Desktop.tsx` and `DesktopIcon.tsx`

- [x] Write Desktop component (display background + grid of icons)
- [x] Write DesktopIcon component (label + icon, click to open window)
- [x] Integrate into App layout

---

### Task 3: Adapt Taskbar — Centered Icons + Neon Style
**Files:** Modify or create `src/components/taskbar/Taskbar.tsx`

- [x] Build Taskbar with centered icons, neon hover, start button, clock
- [x] Ensure interaction with OSContext (`openWindow`, `toggleStartMenu`)

---

### Task 4: Adapt StartMenu — Dark Neon Panel
**Files:** Modify or create `src/components/startmenu/StartMenu.tsx` and `AppTile.tsx`

- [x] Build StartMenu panel with grid of pinned apps and full list
- [x] Apply neon card style (`border-radius: 20px`, glow border)
- [x] Connect to `useOS` (`openWindow`, `toggleStartMenu`)

---

### Task 5: Adapt Window — Integration with Dark Neon Theme
**Files:** Modify `src/components/window/WindowFrame.tsx`

- [x] Adjust colors and shadows in WindowFrame to match neon palette (glow, border, background)
- [x] Verify drag, resize, snap, minimize, maximize work correctly

---

### Task 6: Final Integration — App Layout
**Files:** Modify `src/App.tsx`

- [x] Assemble `Desktop`, `Taskbar`, `StartMenu`, and `WindowFrame` into main layout
- [x] Test full interaction cycle (click icon -> open window, start menu -> open app, drag, close)
- [x] Verify visual consistency across all components

---

## Testing
- [x] Visual verification of dark neon theme in browser
- [x] Interaction tests: desktop icons, start menu, window controls, drag, resize
- [x] Confirm no errors in TypeScript compilation (`npm run typecheck`)

