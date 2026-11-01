import { AppDefinition, FileItem, Wallpaper } from '../types'

/** Hauteur de la barre de menus fixée en haut de l'écran (cf. MenuBar). */
export const MENU_BAR_HEIGHT = 44

export const WALLPAPERS: Wallpaper[] = [
  {
    id: 'bloom-dark',
    name: 'Windows 11 Bloom (Sombre)',
    url: 'https://images.unsplash.com/photo-1618005182384-a83a8bd57fbe?auto=format&fit=crop&w=2560&q=80',
    isDark: true,
    previewColor: '#0a192f',
  },
  {
    id: 'bloom-light',
    name: 'Windows 11 Bloom (Clair)',
    url: 'https://images.unsplash.com/photo-1579546929518-9e396f3cc809?auto=format&fit=crop&w=2560&q=80',
    isDark: false,
    previewColor: '#e0f2fe',
  },
  {
    id: 'glow',
    name: 'Glow Windows 11',
    url: 'https://images.unsplash.com/photo-1634017839464-5c339ebe3cb4?auto=format&fit=crop&w=2560&q=80',
    isDark: true,
    previewColor: '#1e1b4b',
  },
  {
    id: 'flow',
    name: 'Flow Abstraction',
    url: 'https://images.unsplash.com/photo-1550684848-fac1c5b4e853?auto=format&fit=crop&w=2560&q=80',
    isDark: true,
    previewColor: '#0f172a',
  },
  {
    id: 'sunrise',
    name: 'Sunrise Minimal',
    url: 'https://images.unsplash.com/photo-1506744038136-46273834b3fb?auto=format&fit=crop&w=2560&q=80',
    isDark: false,
    previewColor: '#fdba74',
  },
]

export const APP_DEFINITIONS: Record<string, AppDefinition> = {
  explorer: {
    id: 'explorer',
    name: 'Explorateur de fichiers',
    icon: 'folder',
    description: 'Parcourir les dossiers et fichiers du système',
    pinned: true,
    defaultSize: { width: 850, height: 550 },
    minSize: { width: 500, height: 350 },
  },
  photos: {
    id: 'photos',
    name: 'Photos',
    icon: 'image',
    description: 'Afficher et organiser la collection d’images',
    pinned: true,
    defaultSize: { width: 900, height: 620 },
    minSize: { width: 560, height: 400 },
  },
  recycle: {
    id: 'recycle',
    name: 'Corbeille',
    icon: 'recycle',
    description: 'Restaurer ou supprimer définitivement les éléments supprimés',
    pinned: false,
    defaultSize: { width: 720, height: 500 },
    minSize: { width: 480, height: 320 },
  },
  notepad: {
    id: 'notepad',
    name: 'Bloc-notes',
    icon: 'file-text',
    description: 'Éditeur de texte simple et rapide',
    pinned: true,
    defaultSize: { width: 720, height: 480 },
    minSize: { width: 400, height: 280 },
  },
  terminal: {
    id: 'terminal',
    name: 'Terminal Windows',
    icon: 'terminal',
    description: 'Invite de commande PowerShell interactive',
    pinned: true,
    defaultSize: { width: 780, height: 480 },
    minSize: { width: 500, height: 320 },
  },
  calculator: {
    id: 'calculator',
    name: 'Calculatrice',
    icon: 'calculator',
    description: 'Calculs standard et scientifiques',
    pinned: true,
    defaultSize: { width: 340, height: 520 },
    minSize: { width: 320, height: 480 },
  },
  taskmanager: {
    id: 'taskmanager',
    name: 'Gestionnaire des tâches',
    icon: 'activity',
    description: 'Surveiller et arrêter les processus Windows',
    pinned: false,
    defaultSize: { width: 900, height: 620 },
    minSize: { width: 640, height: 420 },
  },
  settings: {
    id: 'settings',
    name: 'Paramètres',
    icon: 'settings',
    description: 'Personnaliser et configurer Windows 11',
    pinned: true,
    defaultSize: { width: 880, height: 580 },
    minSize: { width: 620, height: 400 },
  },
  paint: {
    id: 'paint',
    name: 'Paint',
    icon: 'palette',
    description: 'Dessiner et créer des illustrations',
    pinned: true,
    defaultSize: { width: 820, height: 540 },
    minSize: { width: 500, height: 400 },
  },
}

export const INITIAL_FILES: FileItem[] = [
  // Root folders
  { id: 'f-desktop', name: 'Bureau', type: 'folder', path: 'C:/Users/eemntirey/Desktop', modified: '31/10/2026 07:30' },
  { id: 'f-documents', name: 'Documents', type: 'folder', path: 'C:/Users/eemntirey/Documents', modified: '31/10/2026 07:30' },
  { id: 'f-downloads', name: 'Téléchargements', type: 'folder', path: 'C:/Users/eemntirey/Downloads', modified: '31/10/2026 07:30' },
  { id: 'f-pictures', name: 'Images', type: 'folder', path: 'C:/Users/eemntirey/Pictures', modified: '31/10/2026 07:30' },
  
  // Documents content
  {
    id: 'doc-welcome',
    name: 'Bienvenue sur Windows 11.txt',
    type: 'file',
    extension: 'txt',
    path: 'C:/Users/eemntirey/Documents/Bienvenue sur Windows 11.txt',
    content: `Bienvenue dans la reproduction complète de Windows 11 !

Ce système a été recréé avec soin avec React, TypeScript et l'esthétique Fluent Design :
• Menu Démarrer avec recherche dynamique et liste des applications.
• Barre des tâches moderne centrée avec indicateurs d'état et animations.
• Explorateur de fichiers interactif (navigation, création de dossiers/fichiers).
• Photos avec galerie, zoom, rotation et diaporama.
• Bloc-notes Windows 11 complet avec sauvegarde dans le système de fichiers.
• Terminal simulé avec commandes système documentées.
• Calculatrice Windows 11 réaliste avec historique de calcul.
• Paramètres complets (Thème Sombre/Clair, choix des fonds d'écran, réglages audio/luminosité).
• Centre de contrôle rapide (Quick Settings Wi-Fi, Son, Luminosité, Batterie, Veilleuse).
• Centre de notifications et calendrier mensuel interactif.
• Snap Layouts de fenêtres (glissez ou survolez le bouton d'agrandissement).

Profitez de l'expérience Windows 11 !`,
    size: '1.2 Ko',
    modified: '31/10/2026 07:35',
  },
  {
    id: 'doc-notes',
    name: 'Notes de réunion.txt',
    type: 'file',
    extension: 'txt',
    path: 'C:/Users/eemntirey/Documents/Notes de réunion.txt',
    content: `Objectifs du projet Windows 11 :
1. Expérience utilisateur fluide et réactive.
2. Respect rigoureux des codes graphiques Fluent Design (Mica, acrylique, ombres).
3. Applications fonctionnelles et interconnectées.`,
    size: '420 o',
    modified: '31/10/2026 07:40',
  },
  {
    id: 'doc-config',
    name: 'config.json',
    type: 'file',
    extension: 'json',
    path: 'C:/Users/eemntirey/Documents/config.json',
    content: `{
  "system": "Windows 11 Pro",
  "version": "24H2",
  "build": "26100.1742",
  "theme": "dark",
  "accentColor": "#0078d4"
}`,
    size: '180 o',
    modified: '31/10/2026 07:42',
  },

  // Desktop files
  {
    id: 'desk-readme',
    name: 'Lisez-moi.txt',
    type: 'file',
    extension: 'txt',
    path: 'C:/Users/eemntirey/Desktop/Lisez-moi.txt',
    content: `Double-cliquez sur n'importe quelle icône du bureau ou de la barre des tâches pour ouvrir les applications.
Faites un clic-droit sur le bureau pour afficher le menu contextuel Windows 11.
Survolez le bouton Agrandir d'une fenêtre pour découvrir les Snap Layouts !`,
    size: '350 o',
    modified: '31/10/2026 07:45',
  },
]
