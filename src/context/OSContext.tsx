import React, { createContext, useContext, useState, useEffect, ReactNode, useCallback } from 'react'
import {
  AppId,
  FileItem,
  NotificationItem,
  SnapPosition,
  ThemeMode,
  Wallpaper,
  WindowState,
} from '../types'
import { APP_DEFINITIONS, INITIAL_FILES, MENU_BAR_HEIGHT, WALLPAPERS } from '../data/constants'

export interface RecycleItem extends FileItem {
  originalPath: string
  deletedAt: string
}

interface OSContextType {
  // Windows
  windows: WindowState[]
  activeWindowId: string | null
  openWindow: (appId: AppId, customData?: Record<string, unknown>, titleOverride?: string) => void
  closeWindow: (id: string) => void
  minimizeWindow: (id: string) => void
  maximizeWindow: (id: string) => void
  restoreWindow: (id: string) => void
  bringToFront: (id: string) => void
  updateWindowPosition: (id: string, x: number, y: number) => void
  updateWindowSize: (id: string, width: number, height: number) => void
  snapWindow: (id: string, position: SnapPosition) => void
  toggleWindowMinimize: (id: string) => void

  // Flyouts
  isStartMenuOpen: boolean
  isSearchOpen: boolean
  isActionCenterOpen: boolean
  isCalendarOpen: boolean
  isWidgetsOpen: boolean
  toggleStartMenu: () => void
  toggleSearch: () => void
  toggleActionCenter: () => void
  toggleCalendar: () => void
  toggleWidgets: () => void
  closeAllFlyouts: () => void

  // System Settings
  theme: ThemeMode
  setTheme: (mode: ThemeMode) => void
  toggleTheme: () => void
  wallpaper: Wallpaper
  setWallpaper: (wp: Wallpaper) => void
  accentColor: string
  setAccentColor: (color: string) => void
  volume: number
  setVolume: (vol: number) => void
  isMuted: boolean
  toggleMute: () => void
  brightness: number
  setBrightness: (b: number) => void
  wifiEnabled: boolean
  toggleWifi: () => void
  bluetoothEnabled: boolean
  toggleBluetooth: () => void
  nightLight: boolean
  toggleNightLight: () => void
  batterySaver: boolean
  toggleBatterySaver: () => void

  // Lock & Sleep
  isLocked: boolean
  setIsLocked: (locked: boolean) => void
  isSleeping: boolean
  setIsSleeping: (sleeping: boolean) => void
  lockOS: () => void
  unlockOS: () => void
  sleepOS: () => void
  wakeOS: () => void
  restartOS: () => void
  shutdownOS: () => void

  // Display settings
  showDesktopLabels: boolean
  setShowDesktopLabels: (show: boolean) => void
  toggleDesktopLabels: () => void
  fontSize: number
  setFontSize: (size: number) => void
  resolution: string
  setResolution: (res: string) => void
  refreshRate: string
  setRefreshRate: (rate: string) => void
  focusAssist: boolean
  setFocusAssist: (enabled: boolean) => void
  taskbarAlignment: 'center' | 'left'
  setTaskbarAlignment: (align: 'center' | 'left') => void

  // Privacy
  locationEnabled: boolean
  setLocationEnabled: (enabled: boolean) => void
  cameraEnabled: boolean
  setCameraEnabled: (enabled: boolean) => void
  micEnabled: boolean
  setMicEnabled: (enabled: boolean) => void
  diagnosticData: boolean
  setDiagnosticData: (enabled: boolean) => void

  // Settings app toggles
  notificationsEnabled: boolean
  setNotificationsEnabled: (enabled: boolean) => void
  toggleNotifications: () => void
  airplaneMode: boolean
  toggleAirplaneMode: () => void
  mobileHotspot: boolean
  toggleMobileHotspot: () => void
  gameBarEnabled: boolean
  toggleGameBar: () => void
  gameModeEnabled: boolean
  toggleGameMode: () => void
  audioRecordingEnabled: boolean
  toggleAudioRecording: () => void
  vrrEnabled: boolean
  toggleVRR: () => void
  hardwareGpuScheduling: boolean
  toggleHardwareGpuScheduling: () => void
  magnifierEnabled: boolean
  toggleMagnifier: () => void
  colorFilterEnabled: boolean
  toggleColorFilter: () => void
  highContrastEnabled: boolean
  toggleHighContrast: () => void
  captionsEnabled: boolean
  toggleCaptions: () => void
  audioAlertsEnabled: boolean
  toggleAudioAlerts: () => void
  voiceAccessEnabled: boolean
  toggleVoiceAccess: () => void
  personalizedExperiences: boolean
  togglePersonalizedExperiences: () => void
  activityHistory: boolean
  toggleActivityHistory: () => void
  syncSettings: boolean
  toggleSyncSettings: () => void
  setTimeAutomatically: boolean
  toggleSetTimeAutomatically: () => void
  setTimeZoneAutomatically: boolean
  toggleSetTimeZoneAutomatically: () => void
  lockScreenBackground: boolean
  toggleLockScreenBackground: () => void
  lockScreenAppStatus: boolean
  toggleLockScreenAppStatus: () => void

  // File System
  files: FileItem[]
  recycleItems: RecycleItem[]
  restoreItem: (id: string) => void
  permanentlyDeleteItem: (id: string) => void
  emptyRecycleBin: () => void
  createFolder: (parentPath: string, name: string) => void
  createFile: (parentPath: string, name: string, content?: string) => FileItem
  updateFileContent: (id: string, content: string) => void
  deleteItem: (id: string) => void
  renameItem: (id: string, newName: string) => void

  // Windows Update
  autoUpdate: boolean
  setAutoUpdate: (enabled: boolean) => void

  // Notifications
  notifications: NotificationItem[]
  addNotification: (title: string, message: string, appName: string, icon?: string) => void
  clearNotifications: () => void
  removeNotification: (id: string) => void
};

export const OSContext = createContext<OSContextType | undefined>(undefined)

export const OSProvider: React.FC<{ children: ReactNode }> = ({ children }) => {
  // Windows state - initialized with open File Explorer
  const [windows, setWindows] = useState<WindowState[]>([
    {
      id: 'explorer-init',
      appId: 'explorer',
      title: 'Explorateur de fichiers',
      icon: 'explorer',
      isMinimized: false,
      isMaximized: false,
      zIndex: 10,
      position: { x: 120, y: 50 },
      size: { width: 880, height: 550 },
      customData: { initialPath: 'C:/Users/eemntirey' },
    },
  ])
  const [activeWindowId, setActiveWindowId] = useState<string | null>('explorer-init')
  const [highestZIndex, setHighestZIndex] = useState(11)

  // Flyout states
  const [isStartMenuOpen, setIsStartMenuOpen] = useState(false)
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const [isActionCenterOpen, setIsActionCenterOpen] = useState(false)
  const [isCalendarOpen, setIsCalendarOpen] = useState(false)
  const [isWidgetsOpen, setIsWidgetsOpen] = useState(false)

  // Settings state
  const [theme, setTheme] = useState<ThemeMode>('dark')
  const [wallpaper, setWallpaper] = useState<Wallpaper>(WALLPAPERS[0])
  const [accentColor, setAccentColor] = useState('#0078d4')
  const [volume, setVolume] = useState(70)
  const [isMuted, setIsMuted] = useState(false)
  const [brightness, setBrightness] = useState(90)
  const [wifiEnabled, setWifiEnabled] = useState(true)
  const [bluetoothEnabled, setBluetoothEnabled] = useState(true)
  const [nightLight, setNightLight] = useState(false)
  const [batterySaver, setBatterySaver] = useState(false)

  // Display settings
  const [showDesktopLabels, setShowDesktopLabels] = useState(true)
  const [fontSize, setFontSize] = useState(100)
  const [resolution, setResolution] = useState('1920x1080')
  const [refreshRate, setRefreshRate] = useState('60Hz')
  const [focusAssist, setFocusAssist] = useState(false)
  const [taskbarAlignment, setTaskbarAlignment] = useState<'center' | 'left'>('center')

  // Privacy
  const [locationEnabled, setLocationEnabled] = useState(true)
  const [cameraEnabled, setCameraEnabled] = useState(true)
  const [micEnabled, setMicEnabled] = useState(true)
  const [diagnosticData, setDiagnosticData] = useState(true)

  // Settings app toggles
  const [notificationsEnabled, setNotificationsEnabled] = useState(true)
  const [airplaneMode, setAirplaneMode] = useState(false)
  const [mobileHotspot, setMobileHotspot] = useState(false)
  const [gameBarEnabled, setGameBarEnabled] = useState(true)
  const [gameModeEnabled, setGameModeEnabled] = useState(false)
  const [audioRecordingEnabled, setAudioRecordingEnabled] = useState(true)
  const [vrrEnabled, setVrrEnabled] = useState(false)
  const [hardwareGpuScheduling, setHardwareGpuScheduling] = useState(true)
  const [magnifierEnabled, setMagnifierEnabled] = useState(false)
  const [colorFilterEnabled, setColorFilterEnabled] = useState(false)
  const [highContrastEnabled, setHighContrastEnabled] = useState(false)
  const [captionsEnabled, setCaptionsEnabled] = useState(false)
  const [audioAlertsEnabled, setAudioAlertsEnabled] = useState(false)
  const [voiceAccessEnabled, setVoiceAccessEnabled] = useState(false)
  const [personalizedExperiences, setPersonalizedExperiences] = useState(true)
  const [activityHistory, setActivityHistory] = useState(true)
  const [syncSettings, setSyncSettings] = useState(true)
  const [setTimeAutomatically, setSetTimeAutomatically] = useState(true)
  const [setTimeZoneAutomatically, setSetTimeZoneAutomatically] = useState(true)
  const [lockScreenBackground, setLockScreenBackground] = useState(true)
  const [lockScreenAppStatus, setLockScreenAppStatus] = useState(true)

  // Windows Update
  const [autoUpdate, setAutoUpdate] = useState(true)

  const toggleDesktopLabels = useCallback(() => {
    setShowDesktopLabels(prev => !prev)
  }, [])

  // Lock & Sleep state
  const [isLocked, setIsLocked] = useState(false)
  const [isSleeping, setIsSleeping] = useState(false)

  // File System state
  const [files, setFiles] = useState<FileItem[]>(INITIAL_FILES)
  const [recycleItems, setRecycleItems] = useState<RecycleItem[]>([])

  // Notifications
  const [notifications, setNotifications] = useState<NotificationItem[]>([
    {
      id: 'notif-1',
      title: 'Mise à jour Windows',
      message: 'Windows 11 Pro version 24H2 est prêt et à jour.',
      time: 'Il y a 5 min',
      appName: 'Windows Update',
      unread: true,
    },
    {
      id: 'notif-2',
      title: 'Sécurité Windows',
      message: 'Aucune menace détectée. Votre appareil est protégé.',
      time: 'Il y a 20 min',
      appName: 'Sécurité Windows',
      unread: true,
    },
  ])

  // Sync theme attribute on document
  useEffect(() => {
    document.documentElement.setAttribute('data-theme', theme)
  }, [theme])

  // Keep active window valid after close/minimize
  useEffect(() => {
    setActiveWindowId(prev => {
      if (!prev) return prev
      const activeWindow = windows.find(w => w.id === prev)
      if (activeWindow && !activeWindow.isMinimized) return prev
      const remainingOpen = windows.filter(w => w.id !== prev && !w.isMinimized)
      return remainingOpen.length > 0 ? remainingOpen[remainingOpen.length - 1].id : (windows.find(w => w.id === prev && !w.isMinimized) ? prev : null)
    })
  }, [windows])

  // Close flyouts helper
  const closeAllFlyouts = useCallback(() => {
    setIsStartMenuOpen(false)
    setIsSearchOpen(false)
    setIsActionCenterOpen(false)
    setIsCalendarOpen(false)
    setIsWidgetsOpen(false)
  }, [])

  // Toggle flyout helpers
  const toggleStartMenu = useCallback(() => {
    setIsStartMenuOpen(prev => {
      const next = !prev
      if (next) {
        setIsSearchOpen(false)
        setIsActionCenterOpen(false)
        setIsCalendarOpen(false)
        setIsWidgetsOpen(false)
      }
      return next
    })
  }, [])

  const toggleSearch = useCallback(() => {
    setIsSearchOpen(prev => {
      const next = !prev
      if (next) {
        setIsStartMenuOpen(false)
        setIsActionCenterOpen(false)
        setIsCalendarOpen(false)
        setIsWidgetsOpen(false)
      }
      return next
    })
  }, [])

  const toggleActionCenter = useCallback(() => {
    setIsActionCenterOpen(prev => {
      const next = !prev
      if (next) {
        setIsStartMenuOpen(false)
        setIsSearchOpen(false)
        setIsCalendarOpen(false)
        setIsWidgetsOpen(false)
      }
      return next
    })
  }, [])

  const toggleCalendar = useCallback(() => {
    setIsCalendarOpen(prev => {
      const next = !prev
      if (next) {
        setIsStartMenuOpen(false)
        setIsSearchOpen(false)
        setIsActionCenterOpen(false)
        setIsWidgetsOpen(false)
      }
      return next
    })
  }, [])

  const toggleWidgets = useCallback(() => {
    setIsWidgetsOpen(prev => {
      const next = !prev
      if (next) {
        setIsStartMenuOpen(false)
        setIsSearchOpen(false)
        setIsActionCenterOpen(false)
        setIsCalendarOpen(false)
      }
      return next
    })
  }, [])

  // Window Management
  const bringToFront = useCallback((id: string) => {
    setHighestZIndex(prev => {
      const nextZ = prev + 1
      setWindows(wins =>
        wins.map(w => (w.id === id ? { ...w, zIndex: nextZ, isMinimized: false } : w))
      )
      setActiveWindowId(id)
      return nextZ
    })
  }, [])

  const openWindow = useCallback(
    (appId: AppId, customData?: Record<string, unknown>, titleOverride?: string) => {
      closeAllFlyouts()

      // Multi-instance: always create new window
      const def = APP_DEFINITIONS[appId] || {
        name: 'Application',
        icon: 'app',
        defaultSize: { width: 720, height: 480 },
        minSize: { width: 400, height: 300 },
      }

      // Offset position cascading
      const offset = (windows.length % 8) * 28 + 60
      const newZ = highestZIndex + 1
      setHighestZIndex(newZ)

      const newWindow: WindowState = {
        id: `${appId}-${Date.now()}`,
        appId,
        title: titleOverride || def.name,
        icon: def.icon,
        isMinimized: false,
        isMaximized: false,
        zIndex: newZ,
        position: { x: offset + 60, y: offset },
        size: { ...def.defaultSize },
        customData,
      }

      setWindows(prev => [...prev, newWindow])
      setActiveWindowId(newWindow.id)
    },
    [windows, highestZIndex, closeAllFlyouts, bringToFront]
  )

  const closeWindow = useCallback((id: string) => {
    setWindows(prev => prev.filter(w => w.id !== id))
  }, [])

  const minimizeWindow = useCallback((id: string) => {
    setWindows(prev =>
      prev.map(w => (w.id === id ? { ...w, isMinimized: true } : w))
    )
  }, [])

  const maximizeWindow = useCallback((id: string) => {
    setWindows(prev =>
      prev.map(w => {
        if (w.id !== id) return w
        if (w.isMaximized) {
          // Restore
          return {
            ...w,
            isMaximized: false,
            position: w.previousBounds ? w.previousBounds.position : { x: 80, y: 80 },
            size: w.previousBounds ? w.previousBounds.size : { width: 800, height: 500 },
          }
        } else {
          // Maximize
          return {
            ...w,
            isMaximized: true,
            previousBounds: {
              position: { ...w.position },
              size: { ...w.size },
            },
            position: { x: 0, y: MENU_BAR_HEIGHT },
            size: {
              width: window.innerWidth,
              height: window.innerHeight - 48 - MENU_BAR_HEIGHT, // barre des tâches (48px) + barre de menus
            },
          }
        }
      })
    )
    bringToFront(id)
  }, [bringToFront])

  const restoreWindow = useCallback((id: string) => {
    setWindows(prev =>
      prev.map(w => {
        if (w.id !== id) return w
        return {
          ...w,
          isMaximized: false,
          isMinimized: false,
          position: w.previousBounds ? w.previousBounds.position : { x: 80, y: 80 },
          size: w.previousBounds ? w.previousBounds.size : { width: 800, height: 500 },
        }
      })
    )
    bringToFront(id)
  }, [bringToFront])

  const toggleWindowMinimize = useCallback(
    (id: string) => {
      const target = windows.find(w => w.id === id)
      if (!target) return

      if (target.isMinimized) {
        bringToFront(id)
      } else if (activeWindowId === id) {
        minimizeWindow(id)
      } else {
        bringToFront(id)
      }
    },
    [windows, activeWindowId, bringToFront, minimizeWindow]
  )

  const updateWindowPosition = useCallback((id: string, x: number, y: number) => {
    setWindows(prev =>
      prev.map(w => (w.id === id ? { ...w, position: { x, y } } : w))
    )
  }, [])

  const updateWindowSize = useCallback((id: string, width: number, height: number) => {
    setWindows(prev =>
      prev.map(w => (w.id === id ? { ...w, size: { width, height } } : w))
    )
  }, [])

  const snapWindow = useCallback(
    (id: string, position: SnapPosition) => {
      const screenW = window.innerWidth
      const screenH = window.innerHeight - 48 - MENU_BAR_HEIGHT
      const halfW = Math.floor(screenW / 2)
      const halfH = Math.floor(screenH / 2)

      let newPos = { x: 0, y: MENU_BAR_HEIGHT }
      let newSize = { width: halfW, height: screenH }

      switch (position) {
        case 'left-half':
          newPos = { x: 0, y: MENU_BAR_HEIGHT }
          newSize = { width: halfW, height: screenH }
          break
        case 'right-half':
          newPos = { x: halfW, y: MENU_BAR_HEIGHT }
          newSize = { width: halfW, height: screenH }
          break
        case 'top-left':
          newPos = { x: 0, y: MENU_BAR_HEIGHT }
          newSize = { width: halfW, height: halfH }
          break
        case 'top-right':
          newPos = { x: halfW, y: MENU_BAR_HEIGHT }
          newSize = { width: halfW, height: halfH }
          break
        case 'bottom-left':
          newPos = { x: 0, y: MENU_BAR_HEIGHT + halfH }
          newSize = { width: halfW, height: halfH }
          break
        case 'bottom-right':
          newPos = { x: halfW, y: MENU_BAR_HEIGHT + halfH }
          newSize = { width: halfW, height: halfH }
          break
      }

      setWindows(prev =>
        prev.map(w =>
          w.id === id
            ? {
                ...w,
                isMaximized: false,
                position: newPos,
                size: newSize,
              }
            : w
        )
      )
      bringToFront(id)
    },
    [bringToFront]
  )

  // Virtual File System Operations
  const createFolder = useCallback((parentPath: string, name: string) => {
    const newFolder: FileItem = {
      id: `folder-${Date.now()}`,
      name,
      type: 'folder',
      path: `${parentPath}/${name}`.replace(/\/+/g, '/'),
      modified: new Date().toLocaleString('fr-FR'),
    }
    setFiles(prev => [...prev, newFolder])
  }, [])

  const createFile = useCallback((parentPath: string, name: string, content = '') => {
    const ext = name.includes('.') ? name.split('.').pop() : 'txt'
    const newFile: FileItem = {
      id: `file-${Date.now()}`,
      name,
      type: 'file',
      extension: ext,
      path: `${parentPath}/${name}`.replace(/\/+/g, '/'),
      content,
      size: `${content.length} o`,
      modified: new Date().toLocaleString('fr-FR'),
    }
    setFiles(prev => [...prev, newFile])
    return newFile
  }, [])

  const updateFileContent = useCallback((id: string, content: string) => {
    setFiles(prev =>
      prev.map(f =>
        f.id === id
          ? {
              ...f,
              content,
              size: `${content.length} o`,
              modified: new Date().toLocaleString('fr-FR'),
            }
          : f
      )
    )
  }, [])

  const deleteItem = useCallback((id: string) => {
    const target = files.find(item => item.id === id)
    if (!target) return
    const folderPrefix = target.path + '/'
    const removed = files.filter(item => item.id === id || (target.type === 'folder' && item.path.startsWith(folderPrefix)))
    const deletedAt = new Date().toLocaleString('fr-FR')
    setRecycleItems(current => [...current, ...removed.map(item => ({ ...item, originalPath: item.path, deletedAt }))])
    setFiles(current => current.filter(item => !removed.some(removedItem => removedItem.id === item.id)))
  }, [files])

  const restoreItem = useCallback((id: string) => {
    const target = recycleItems.find(item => item.id === id)
    if (!target) return
    const folderPrefix = target.originalPath + '/'
    const restoring = recycleItems.filter(item => item.id === id || (target.type === 'folder' && item.originalPath.startsWith(folderPrefix)))
    setFiles(current => [...current, ...restoring.map(({ originalPath, deletedAt, ...item }) => ({ ...item, path: originalPath }))])
    setRecycleItems(current => current.filter(item => !restoring.some(restored => restored.id === item.id)))
  }, [recycleItems])

  const permanentlyDeleteItem = useCallback((id: string) => {
    const target = recycleItems.find(item => item.id === id)
    if (!target) return
    const folderPrefix = target.originalPath + '/'
    const removing = recycleItems.filter(item => item.id === id || (target.type === 'folder' && item.originalPath.startsWith(folderPrefix)))
    setRecycleItems(current => current.filter(item => !removing.some(removedItem => removedItem.id === item.id)))
  }, [recycleItems])

  const emptyRecycleBin = useCallback(() => setRecycleItems([]), [])

  const renameItem = useCallback((id: string, newName: string) => {
    setFiles(prev => {
      const target = prev.find(f => f.id === id)
      if (!target) return prev
      const oldPath = target.path
      const parts = oldPath.split('/')
      parts.pop()
      const newPath = [...parts, newName].join('/')
      const folderPath = oldPath + '/'
      return prev.map(f => {
        if (f.id === id) return { ...f, name: newName, path: newPath, modified: new Date().toLocaleString('fr-FR') }
        if (f.path.startsWith(folderPath)) return { ...f, path: f.path.replace(oldPath, newPath), modified: new Date().toLocaleString('fr-FR') }
        return f
      })
    })
  }, [])

  // Notifications Operations
  const addNotification = useCallback(
    (title: string, message: string, appName: string, icon?: string) => {
      const newNotif: NotificationItem = {
        id: `notif-${Date.now()}`,
        title,
        message,
        time: 'À l’instant',
        appName,
        icon,
        unread: true,
      }
      setNotifications(prev => [newNotif, ...prev])
    },
    []
  )

  const clearNotifications = useCallback(() => {
    setNotifications([])
  }, [])

  const removeNotification = useCallback((id: string) => {
    setNotifications(prev => prev.filter(n => n.id !== id))
  }, [])

  // Power actions
  const toggleTheme = useCallback(() => {
    setTheme(prev => (prev === 'dark' ? 'light' : 'dark'))
  }, [])

  const toggleMute = useCallback(() => {
    setIsMuted(prev => !prev)
  }, [])

  const toggleWifi = useCallback(() => {
    setWifiEnabled(prev => !prev)
  }, [])

  const toggleBluetooth = useCallback(() => {
    setBluetoothEnabled(prev => !prev)
  }, [])

  const toggleNightLight = useCallback(() => {
    setNightLight(prev => !prev)
  }, [])

  const toggleBatterySaver = useCallback(() => {
    setBatterySaver(prev => !prev)
  }, [])

  // Settings app toggles
  const toggleNotifications = useCallback(() => {
    setNotificationsEnabled(prev => !prev)
  }, [])

  const toggleAirplaneMode = useCallback(() => {
    setAirplaneMode(prev => !prev)
  }, [])

  const toggleMobileHotspot = useCallback(() => {
    setMobileHotspot(prev => !prev)
  }, [])

  const toggleGameBar = useCallback(() => {
    setGameBarEnabled(prev => !prev)
  }, [])

  const toggleGameMode = useCallback(() => {
    setGameModeEnabled(prev => !prev)
  }, [])

  const toggleAudioRecording = useCallback(() => {
    setAudioRecordingEnabled(prev => !prev)
  }, [])

  const toggleVRR = useCallback(() => {
    setVrrEnabled(prev => !prev)
  }, [])

  const toggleHardwareGpuScheduling = useCallback(() => {
    setHardwareGpuScheduling(prev => !prev)
  }, [])

  const toggleMagnifier = useCallback(() => {
    setMagnifierEnabled(prev => !prev)
  }, [])

  const toggleColorFilter = useCallback(() => {
    setColorFilterEnabled(prev => !prev)
  }, [])

  const toggleHighContrast = useCallback(() => {
    setHighContrastEnabled(prev => !prev)
  }, [])

  const toggleCaptions = useCallback(() => {
    setCaptionsEnabled(prev => !prev)
  }, [])

  const toggleAudioAlerts = useCallback(() => {
    setAudioAlertsEnabled(prev => !prev)
  }, [])

  const toggleVoiceAccess = useCallback(() => {
    setVoiceAccessEnabled(prev => !prev)
  }, [])

  const togglePersonalizedExperiences = useCallback(() => {
    setPersonalizedExperiences(prev => !prev)
  }, [])

  const toggleActivityHistory = useCallback(() => {
    setActivityHistory(prev => !prev)
  }, [])

  const toggleSyncSettings = useCallback(() => {
    setSyncSettings(prev => !prev)
  }, [])

  const toggleSetTimeAutomatically = useCallback(() => {
    setSetTimeAutomatically(prev => !prev)
  }, [])

  const toggleSetTimeZoneAutomatically = useCallback(() => {
    setSetTimeZoneAutomatically(prev => !prev)
  }, [])

  const toggleLockScreenBackground = useCallback(() => {
    setLockScreenBackground(prev => !prev)
  }, [])

  const toggleLockScreenAppStatus = useCallback(() => {
    setLockScreenAppStatus(prev => !prev)
  }, [])

  const lockOS = useCallback(() => {
    closeAllFlyouts()
    setIsLocked(true)
  }, [closeAllFlyouts])

  const unlockOS = useCallback(() => {
    setIsLocked(false)
  }, [])

  const sleepOS = useCallback(() => {
    closeAllFlyouts()
    setIsSleeping(true)
  }, [closeAllFlyouts])

  const wakeOS = useCallback(() => {
    setIsSleeping(false)
  }, [])

  const restartOS = useCallback(() => {
    closeAllFlyouts()
    setWindows([])
    setIsLocked(true)
    setTimeout(() => {
      setIsLocked(false)
    }, 1500)
  }, [closeAllFlyouts])

  const shutdownOS = useCallback(() => {
    closeAllFlyouts()
    setWindows([])
    setIsSleeping(true)
  }, [closeAllFlyouts])

  return (
    <OSContext.Provider
      value={{
        windows,
        activeWindowId,
        openWindow,
        closeWindow,
        minimizeWindow,
        maximizeWindow,
        restoreWindow,
        bringToFront,
        updateWindowPosition,
        updateWindowSize,
        snapWindow,
        toggleWindowMinimize,

        isStartMenuOpen,
        isSearchOpen,
        isActionCenterOpen,
        isCalendarOpen,
        isWidgetsOpen,
        toggleStartMenu,
        toggleSearch,
        toggleActionCenter,
        toggleCalendar,
        toggleWidgets,
        closeAllFlyouts,

        theme,
        setTheme,
        toggleTheme,
        wallpaper,
        setWallpaper,
        accentColor,
        setAccentColor,
        volume,
        setVolume,
        isMuted,
        toggleMute,
        brightness,
        setBrightness,
        wifiEnabled,
        toggleWifi,
        bluetoothEnabled,
        toggleBluetooth,
        nightLight,
        toggleNightLight,
        batterySaver,
        toggleBatterySaver,
        showDesktopLabels,
        setShowDesktopLabels,
        toggleDesktopLabels,
        fontSize,
        setFontSize,
        focusAssist,
        setFocusAssist,
        resolution,
        setResolution,
        refreshRate,
        setRefreshRate,
        taskbarAlignment,
        setTaskbarAlignment,
        autoUpdate,
        setAutoUpdate,
        locationEnabled,
        setLocationEnabled,
        cameraEnabled,
        setCameraEnabled,
        micEnabled,
        setMicEnabled,
        diagnosticData,
        setDiagnosticData,

        // Settings app toggles
        notificationsEnabled,
        setNotificationsEnabled,
        toggleNotifications,
        airplaneMode,
        toggleAirplaneMode,
        mobileHotspot,
        toggleMobileHotspot,
        gameBarEnabled,
        toggleGameBar,
        gameModeEnabled,
        toggleGameMode,
        audioRecordingEnabled,
        toggleAudioRecording,
        vrrEnabled,
        toggleVRR,
        hardwareGpuScheduling,
        toggleHardwareGpuScheduling,
        magnifierEnabled,
        toggleMagnifier,
        colorFilterEnabled,
        toggleColorFilter,
        highContrastEnabled,
        toggleHighContrast,
        captionsEnabled,
        toggleCaptions,
        audioAlertsEnabled,
        toggleAudioAlerts,
        voiceAccessEnabled,
        toggleVoiceAccess,
        personalizedExperiences,
        togglePersonalizedExperiences,
        activityHistory,
        toggleActivityHistory,
        syncSettings,
        toggleSyncSettings,
        setTimeAutomatically,
        toggleSetTimeAutomatically,
        setTimeZoneAutomatically,
        toggleSetTimeZoneAutomatically,
        lockScreenBackground,
        toggleLockScreenBackground,
        lockScreenAppStatus,
        toggleLockScreenAppStatus,

        isLocked,
        setIsLocked,
        isSleeping,
        setIsSleeping,
        lockOS,
        unlockOS,
        sleepOS,
        wakeOS,
        restartOS,
        shutdownOS,

        files,
        recycleItems,
        restoreItem,
        permanentlyDeleteItem,
        emptyRecycleBin,
        createFolder,
        createFile,
        updateFileContent,
        deleteItem,
        renameItem,

        notifications,
        addNotification,
        clearNotifications,
        removeNotification,
      } as OSContextType}
    >
      {children}
    </OSContext.Provider>
  )
}

export const useOS = () => {
  const context = useContext(OSContext)
  if (!context) {
    throw new Error('useOS must be used within an OSProvider')
  }
  return context
}
