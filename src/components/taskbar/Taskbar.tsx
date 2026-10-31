import React, { useState, useEffect, useCallback } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon, WindowsLogo } from '../common/FluentIcon'
import { TaskbarIcon } from './TaskbarIcon'
import { AppId } from '../../types'

export const Taskbar: React.FC = () => {
  const {
    windows,
    activeWindowId,
    openWindow,
    toggleWindowMinimize,
    isStartMenuOpen,
    toggleStartMenu,
    isCalendarOpen,
    toggleCalendar,
    notifications,
    taskbarAlignment,
  } = useOS()

  const [currentTime, setCurrentTime] = useState(new Date())
  const [showCustomize, setShowCustomize] = useState(false)
  const [customizeAppIds, setCustomizeAppIds] = useState<AppId[]>([
    'edge', 'explorer', 'notepad', 'terminal', 'calculator', 'paint', 'settings'
  ])

  const [hoveredAppId, setHoveredAppId] = useState<string | null>(null)
  const [showAppGroup, setShowAppGroup] = useState(false)

  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  const formattedTime = currentTime.toLocaleTimeString('fr-FR', {
    hour: '2-digit',
    minute: '2-digit',
  })

  const formattedDate = currentTime.toLocaleDateString('fr-FR', {
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
  })

  const allTaskbarAppIds = Array.from(
    new Set([...customizeAppIds, ...windows.map(w => w.appId)])
  )

  const unreadCount = notifications.filter(n => n.unread).length

  // Magnification logic: compute scale for each icon based on distance from hovered
  const getIconScale = useCallback(
    (_appId: string, index: number) => {
      if (!hoveredAppId) return 1
      const hoveredIndex = allTaskbarAppIds.findIndex(a => a === hoveredAppId)
      if (hoveredIndex === -1) return 1
      const distance = Math.abs(index - hoveredIndex)
      if (distance === 0) return 1.55
      if (distance === 1) return 1.2
      if (distance === 2) return 1.08
      return 1
    },
    [hoveredAppId, allTaskbarAppIds]
  )

  return (
    <div className="dock-container" style={{ justifyContent: taskbarAlignment === 'left' ? 'flex-start' : 'center' }}>
      {/* Left separator + Start (Launchpad style) */}
      <button
        onClick={toggleStartMenu}
        title="Démarrer"
        className="dock-icon"
        style={{
          width: 52,
          height: 52,
          borderRadius: '50%',
          border: isStartMenuOpen ? '1.5px solid rgba(139,92,246,0.5)' : '1.5px solid transparent',
          backgroundColor: isStartMenuOpen ? 'rgba(139,92,246,0.15)' : 'transparent',
          boxShadow: isStartMenuOpen ? '0 0 18px rgba(139,92,246,0.5)' : 'none',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          transition: 'all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1)',
          animation: isStartMenuOpen ? 'bounce-click 0.35s ease' : 'none',
        }}
        onMouseEnter={() => setHoveredAppId('start')}
        onMouseLeave={() => setHoveredAppId(null)}
      >
        <WindowsLogo size={28} />
      </button>

      <button
        onClick={() => setShowCustomize(!showCustomize)}
        title="Personnaliser"
        style={{ width: 36, height: 36, borderRadius: '50%', border: '1px solid rgba(255,255,255,0.1)', background: 'rgba(255,255,255,0.03)', color: '#777', cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 14, fontWeight: 700, transition: 'all 0.15s' }}
      >+
      </button>

      {showCustomize && (
        <div style={{ position: 'absolute', bottom: 60, left: '50%', transform: 'translateX(-50%)', background: 'rgba(10,10,14,0.98)', border: '1px solid rgba(255,255,255,0.15)', borderRadius: 12, padding: '14px 16px', zIndex: 99999, boxShadow: 'none', minWidth: 220 }}>
          <div style={{ fontWeight: 600, color: '#fff', marginBottom: 8, fontSize: 13 }}>Personnaliser la barre</div>
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
            {['edge', 'explorer', 'notepad', 'terminal', 'calculator', 'paint', 'settings'].map(app => (
              <button key={app} onClick={() => { if (!customizeAppIds.includes(app as AppId)) setCustomizeAppIds([...customizeAppIds, app as AppId]); else setCustomizeAppIds(customizeAppIds.filter(a => a !== app)) }} style={{ padding: '4px 10px', borderRadius: 6, border: customizeAppIds.includes(app as AppId) ? '1px solid rgba(255,255,255,0.3)' : '1px solid rgba(255,255,255,0.1)', background: customizeAppIds.includes(app as AppId) ? 'rgba(255,255,255,0.08)' : 'transparent', color: '#ccc', cursor: 'pointer', fontSize: 11 }}>{app}</button>
            ))}
          </div>
          <button onClick={() => setShowCustomize(false)} style={{ marginTop: 10, padding: '4px 12px', border: 'none', borderRadius: 6, background: 'rgba(255,255,255,0.1)', color: '#fff', cursor: 'pointer', fontSize: 11, width: '100%' }}>Fermer</button>
        </div>
      )}

      {/* App Icons */}
      {allTaskbarAppIds.map((appId, index) => {
        const openWin = windows.find(w => w.appId === appId)
        const isOpen = Boolean(openWin)
        const isActive = Boolean(openWin && activeWindowId === openWin.id && !openWin.isMinimized)
        const scale = getIconScale(appId, index)

        return (
          <div key={appId} onContextMenu={(e: React.MouseEvent) => { e.preventDefault(); setCustomizeAppIds(customizeAppIds.filter(a => a !== appId)) }}>
            <TaskbarIcon
            key={appId}
            id={appId}
            icon={appId}
            title={appId}
            isOpen={isOpen}
            isActive={isActive}
            scale={scale}
            onMouseEnter={() => setHoveredAppId(appId)}
            onMouseLeave={() => setHoveredAppId(null)}
            onClick={() => { if (openWin) toggleWindowMinimize(openWin.id); else openWindow(appId) }}
          />
          </div>
        )
      })}


      {/* App Group Block */}
      <div style={{ position: 'relative', display: 'flex', alignItems: 'center', justifyContent: 'center', height: 52 }}>
        <button
          title="Dossier d'apps"
          className="dock-icon"
          onClick={() => setShowAppGroup(v => !v)}
          style={{ width: 52, height: 52, borderRadius: '50%', border: '1.5px solid transparent', backgroundColor: showAppGroup ? 'rgba(139,92,246,0.15)' : 'transparent', boxShadow: showAppGroup ? '0 0 12px rgba(139,92,246,0.4)' : 'none', display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'all 0.2s', cursor: 'pointer' }}
        >
          <FluentIcon name="folder" size={24} color="#f0f0f5" />
        </button>
        {showAppGroup && (
          <div style={{ position: 'absolute', bottom: '60px', left: '-80px', width: 220, background: '#1a1a1a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px', padding: '12px', boxShadow: '0 8px 20px rgba(0,0,0,0.2)', zIndex: 1000, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: '8px' }}>
            {customizeAppIds.map(appId => (
              <button key={appId} onClick={() => { openWindow(appId); setShowAppGroup(false); }} title={appId} style={{ background: 'none', border: 'none', display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '4px', cursor: 'pointer', color: '#ccc', fontSize: '11px' }}>
                <FluentIcon name={appId} size={22} />
                <span style={{ fontSize: '10px', whiteSpace: 'nowrap' }}>{appId}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Divider before trash/bin area */}
      <div className="dock-divider" />

      {/* Trash / Bin icon */}
      <button
        title="Corbeille"
        className="dock-icon"
        style={{
          width: 52,
          height: 52,
          borderRadius: '50%',
          border: '1.5px solid transparent',
          backgroundColor: 'transparent',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          transition: 'all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1)',
        }}
        onMouseEnter={() => setHoveredAppId('trash')}
        onMouseLeave={() => setHoveredAppId(null)}
      >
        <FluentIcon name="trash" size={24} color="#a0a0c0" />
      </button>

      {/* System tray area on the right side of the dock */}
      <div style={{ width: '16px' }} />

      {/* Clock */}
      <button
        onClick={toggleCalendar}
        title="Heure et date"
        className="dock-icon-clock"
        style={{
          padding: '6px 10px',
          borderRadius: '16px',
          border: isCalendarOpen ? '1px solid rgba(139,92,246,0.3)' : '1px solid transparent',
          backgroundColor: isCalendarOpen ? 'rgba(139,92,246,0.12)' : 'rgba(255,255,255,0.05)',
          color: 'var(--text-primary)',
          cursor: 'pointer',
          transition: 'all 0.2s ease',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'flex-end',
          lineHeight: '1.2',
        }}
      >
        <span style={{ fontSize: '11px', fontWeight: 700, letterSpacing: '0.02em' }}>{formattedTime}</span>
        <span style={{ fontSize: '9px', color: 'var(--text-secondary)' }}>{formattedDate}</span>
      </button>

      {/* Notification bell */}
      <button
        onClick={toggleCalendar}
        title="Notifications"
        className="dock-icon"
        style={{
          width: 40,
          height: 40,
          borderRadius: '50%',
          border: unreadCount > 0 ? '1.5px solid rgba(0,212,255,0.4)' : '1.5px solid transparent',
          backgroundColor: unreadCount > 0 ? 'rgba(0,212,255,0.1)' : 'transparent',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          transition: 'all 0.2s ease',
          position: 'relative',
        }}
      >
        <FluentIcon name="bell" size={18} color={unreadCount > 0 ? '#00d4ff' : '#a0a0c0'} />
        {unreadCount > 0 && (
          <span
            style={{
              position: 'absolute',
              top: 4,
              right: 4,
              width: 7,
              height: 7,
              borderRadius: '50%',
              backgroundColor: '#00d4ff',
              boxShadow: '0 0 6px rgba(0,212,255,0.6)',
            }}
          />
        )}
      </button>
    </div>
  )
}
