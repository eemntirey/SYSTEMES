export type AppId =
  | 'explorer'
  | 'notepad'
  | 'terminal'
  | 'calculator'
  | 'settings'
  | 'paint'
  | 'photos'
  | 'recycle'
  | 'widgets'
  | 'taskmanager'

export type SnapPosition =
  | 'left-half'
  | 'right-half'
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'

export interface WindowState {
  id: string
  appId: AppId
  title: string
  icon: string
  isMinimized: boolean
  isMaximized: boolean
  zIndex: number
  position: { x: number; y: number }
  size: { width: number; height: number }
  previousBounds?: {
    position: { x: number; y: number }
    size: { width: number; height: number }
  }
  customData?: Record<string, unknown>
}

export interface FileItem {
  id: string
  name: string
  type: 'folder' | 'file'
  extension?: string
  content?: string
  path: string
  size?: string
  modified: string
  icon?: string
}

export type ThemeMode = 'dark' | 'light'

export interface Wallpaper {
  id: string
  name: string
  url: string
  isDark: boolean
  previewColor: string
}

export interface NotificationItem {
  id: string
  title: string
  message: string
  time: string
  appName: string
  icon?: string
  unread: boolean
}

export interface AppDefinition {
  id: AppId
  name: string
  icon: string
  description: string
  pinned: boolean
  defaultSize: { width: number; height: number }
  minSize: { width: number; height: number }
}
