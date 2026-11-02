import { AppDefinition, FileItem, Wallpaper } from '../types'

/**
 * Hauteur réservée en haut de l'écran.
 * Windows 11 n'a pas de barre de menus haute (contrairement au shell macOS) :
 * les fenêtres occupent le bord supérieur. Conserve à 0 pour ne pas casser
 * les calculs de positionnement existants (maximize/snap/drag).
 */
export const MENU_BAR_HEIGHT = 0

/** Hauteur de la barre des tâches Windows 11 (bas de l'écran). */
export const TASKBAR_HEIGHT = 48

export const WALLPAPERS: Wallpaper[] = [
  {
    id: 'bloom-dark',
    name: 'Windows 11 Bloom (Sombre)',
    url: '/wallpapers/bloom-dark.svg',
    isDark: true,
    previewColor: '#0b2247',
  },
  {
    id: 'bloom-light',
    name: 'Windows 11 Bloom (Clair)',
    url: '/wallpapers/bloom-light.svg',
    isDark: false,
    previewColor: '#bcd9f7',
  },
  {
    id: 'glow',
    name: 'Windows 11 Glow',
    url: '/wallpapers/bloom-dark.svg',
    isDark: true,
    previewColor: '#123a6b',
  },
  {
    id: 'flow',
    name: 'Windows 11 Flow',
    url: '/wallpapers/bloom-light.svg',
    isDark: true,
    previewColor: '#0f172a',
  },
  {
    id: 'sunrise',
    name: 'Windows 11 Aurore',
    url: '/wallpapers/bloom-light.svg',
    isDark: false,
    previewColor: '#8fb6e6',
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
  services: {
    id: 'services',
    name: 'Services',
    icon: 'activity',
    description: 'Consulter et contrôler les services Windows',
    pinned: false,
    defaultSize: { width: 960, height: 640 },
    minSize: { width: 700, height: 420 },
  },
}

export const INITIAL_FILES: FileItem[] = [
  { id: 'f-desktop', name: 'Bureau', type: 'folder', path: 'C:/Users/eemntirey/Desktop', modified: '31/10/2026 07:30' },
  { id: 'f-documents', name: 'Documents', type: 'folder', path: 'C:/Users/eemntirey/Documents', modified: '31/10/2026 07:30' },
  { id: 'f-downloads', name: 'Téléchargements', type: 'folder', path: 'C:/Users/eemntirey/Downloads', modified: '31/10/2026 07:30' },
  { id: 'f-pictures', name: 'Images', type: 'folder', path: 'C:/Users/eemntirey/Pictures', modified: '31/10/2026 07:30' },
  {
    id: 'doc-welcome',
    name: 'Bienvenue sur Windows 11.txt',
    type: 'file',
    extension: 'txt',
    path: 'C:/Users/eemntirey/Documents/Bienvenue sur Windows 11.txt',
    content: 'Bienvenue dans SYSTEMES.',
    size: '1.2 Ko',
    modified: '31/10/2026 07:35',
  },
  {
    id: 'desk-readme',
    name: 'Lisez-moi.txt',
    type: 'file',
    extension: 'txt',
    path: 'C:/Users/eemntirey/Desktop/Lisez-moi.txt',
    content: 'Double-cliquez sur une icône pour ouvrir une application.',
    size: '350 o',
    modified: '31/10/2026 07:45',
  },
]
