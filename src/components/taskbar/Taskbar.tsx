import React, { useState, useEffect, useCallback } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon, WindowsLogo } from '../common/FluentIcon'
import { TaskbarIcon } from './TaskbarIcon'
import { APP_DEFINITIONS } from '../../data/constants'
import { AppId } from '../../types'

/** Apps pinned by default on the Windows 11 taskbar. */
const PINNED: AppId[] = ['explorer', 'notepad', 'terminal', 'calculator', 'settings', 'photos', 'paint']

export const Taskbar: React.FC = () => {
  const {
    windows,
    activeWindowId,
    openWindow,
    bringToFront,
    isStartMenuOpen,
    toggleStartMenu,
    isSearchOpen,
    toggleSearch,
    isActionCenterOpen,
    toggleActionCenter,
    isCalendarOpen,
    toggleCalendar,
    isWidgetsOpen,
    toggleWidgets,
    isTaskViewOpen,
    toggleTaskView,
    closeAllFlyouts,
    notifications,
    taskbarAlignment,
    volume,
    isMuted,
    wifiEnabled,
    minimizeWindow,
  } = useOS()

  const [currentTime, setCurrentTime] = useState(new Date())
  const [showTray, setShowTray] = useState(false)

  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  const formattedTime = currentTime.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })
  const formattedDate = currentTime.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', year: 'numeric' })
  const unreadCount = notifications.filter(n => n.unread).length

  // Union of pinned apps + apps with open windows, in stable order.
  const openAppIds = new Set(windows.map(w => w.appId))
  const appIds = Array.from(new Set([...PINNED, ...openAppIds]))

  const handleAppClick = useCallback(
    (appId: AppId) => {
      const appWindows = windows.filter(w => w.appId === appId)
      if (appWindows.length === 0) {
        openWindow(appId)
        return
      }
      const activeOfApp = appWindows.find(w => w.id === activeWindowId)
      if (activeOfApp && !activeOfApp.isMinimized) {
        minimizeWindow(activeOfApp.id)
      } else {
        bringToFront((activeOfApp ?? appWindows[0]).id)
      }
    },
    [windows, activeWindowId, openWindow, minimizeWindow, bringToFront]
  )

  // "Afficher le bureau" : minimise tout (et restaure au clic suivant si tout est minimisé).
  const allMinimized = windows.length > 0 && windows.every(w => w.isMinimized)
  const handleShowDesktop = () => {
    if (allMinimized) {
      windows.forEach(w => bringToFront(w.id))
    } else {
      windows.forEach(w => minimizeWindow(w.id))
    }
  }

  const volumeIcon = isMuted || volume === 0 ? 'volume-x' : 'volume'

  return (
    <div className="taskbar-w11 obsidian-taskbar" role="toolbar" aria-label="Barre des tâches">
      {/* ===== Centre : Démarrer, Recherche, Task View, Widgets, Copilot, apps ===== */}
      <div className={`taskbar-center${taskbarAlignment === 'left' ? ' align-left' : ''}`}>
        {/* Démarrer */}
        <button
          className={`taskbar-btn${isStartMenuOpen ? ' open' : ''}`}
          onClick={toggleStartMenu}
          aria-label="Démarrer"
          aria-pressed={isStartMenuOpen}
          title="Démarrer"
          style={{ minWidth: 44 }}
        >
          <WindowsLogo size={22} />
        </button>

        {/* Recherche (boîte façon Win11) */}
        <button
          className={`taskbar-btn${isSearchOpen ? ' open' : ''}`}
          onClick={toggleSearch}
          aria-label="Rechercher"
          aria-pressed={isSearchOpen}
          title="Rechercher"
          style={{ width: 164, justifyContent: 'flex-start', gap: 8, padding: '0 12px', borderRadius: 8 }}
        >
          <FluentIcon name="search" size={16} color="var(--text-secondary)" />
          <span style={{ fontSize: 12.5, color: 'var(--text-secondary)' }}>Rechercher</span>
        </button>

        {/* Task View */}
        <button
          className={`taskbar-btn${isTaskViewOpen ? ' open' : ''}`}
          onClick={toggleTaskView}
          aria-label="Vue des tâches"
          aria-pressed={isTaskViewOpen}
          title="Vue des tâches"
        >
          <FluentIcon name="taskview" size={21} />
        </button>

        {/* Widgets */}
        <button
          className={`taskbar-btn${isWidgetsOpen ? ' open' : ''}`}
          onClick={toggleWidgets}
          aria-label="Widgets"
          aria-pressed={isWidgetsOpen}
          title="Widgets"
        >
          <FluentIcon name="widgets" size={21} />
        </button>

        {/* Copilot */}
        <button
          className="taskbar-btn"
          onClick={() => {
            closeAllFlyouts()
            openWindow('settings')
          }}
          aria-label="Copilot"
          title="Copilot"
        >
          <FluentIcon name="copilot" size={22} />
        </button>

        {/* Séparateur discret */}
        <div style={{ width: 1, height: 24, background: 'var(--taskbar-border)', margin: '0 4px' }} />

        {/* Apps épinglées / ouvertes */}
        {appIds.map(appId => {
          const appWindows = windows.filter(w => w.appId === appId)
          const isOpen = appWindows.length > 0
          const isActive = appWindows.some(w => w.id === activeWindowId && !w.isMinimized)
          const def = APP_DEFINITIONS[appId]
          return (
            <TaskbarIcon
              key={appId}
              id={appId}
              icon={def?.icon ?? appId}
              title={def?.name ?? appId}
              isOpen={isOpen}
              isActive={isActive}
              onClick={() => handleAppClick(appId)}
            />
          )
        })}
      </div>

      {/* ===== Droite : system tray ===== */}
      <div className="taskbar-tray">
        {/* Icônes cachées (chevron) */}
        <button
          className="taskbar-btn"
          onClick={() => setShowTray(v => !v)}
          aria-label="Afficher les icônes cachées"
          title="Afficher les icônes cachées"
          style={{ minWidth: 28, padding: '0 4px' }}
        >
          <FluentIcon name="chevron-up" size={15} color="var(--text-secondary)" />
        </button>

        {showTray && (
          <div className="tray-flyout">
            <button className="taskbar-btn" title="Sécurité Windows"><FluentIcon name="shield" size={17} /></button>
            <button className="taskbar-btn" title="OneDrive"><FluentIcon name="cloud" size={17} /></button>
            <button className="taskbar-btn" title="NVIDIA"><FluentIcon name="palette" size={17} /></button>
          </div>
        )}

        {/* Réseau + Son + Batterie → ouvre le centre de contrôle rapide */}
        <button
          className={`taskbar-btn${isActionCenterOpen ? ' open' : ''}`}
          onClick={toggleActionCenter}
          aria-label="Réseau, son, batterie"
          aria-pressed={isActionCenterOpen}
          title="Réseau, son, batterie"
          style={{ gap: 8, padding: '0 10px' }}
        >
          <FluentIcon name={wifiEnabled ? 'wifi' : 'wifi-off'} size={16} color="var(--text-primary)" />
          <FluentIcon name={volumeIcon} size={16} color="var(--text-primary)" />
          <FluentIcon name="battery" size={16} color="var(--text-primary)" />
        </button>

        {/* Micro (indicateur d'état) */}
        <button className="taskbar-btn" title="Microphone" aria-label="Microphone" style={{ minWidth: 28, padding: '0 4px' }}>
          <FluentIcon name="mic" size={15} color="var(--text-secondary)" />
        </button>

        {/* Horloge → ouvre calendrier / notifications */}
        <button
          className="taskbar-clock"
          onClick={toggleCalendar}
          aria-label="Heure, date et notifications"
          aria-pressed={isCalendarOpen}
          title={`${formattedDate} ${formattedTime}`}
          style={{ position: 'relative' }}
        >
          <span style={{ fontSize: 12, fontWeight: 400 }}>{formattedTime}</span>
          <span style={{ fontSize: 12, fontWeight: 400 }}>{formattedDate}</span>
          {unreadCount > 0 && (
            <span
              style={{
                position: 'absolute', top: 4, right: 2, width: 6, height: 6,
                borderRadius: '50%', background: 'var(--win-accent)', boxShadow: '0 0 6px var(--win-accent)',
              }}
            />
          )}
        </button>

        {/* Notifications (bouton flèche) */}
        <button
          className={`taskbar-btn${isCalendarOpen ? ' open' : ''}`}
          onClick={toggleCalendar}
          aria-label="Notifications"
          aria-pressed={isCalendarOpen}
          title="Notifications"
          style={{ minWidth: 28, padding: '0 4px' }}
        >
          <FluentIcon name="bell" size={15} color="var(--text-secondary)" />
        </button>

        {/* Afficher le bureau (liseré tout à droite) */}
        <button
          className="taskbar-showdesktop"
          onClick={handleShowDesktop}
          aria-label="Afficher le bureau"
          title="Afficher le bureau"
        />
      </div>
    </div>
  )
}
