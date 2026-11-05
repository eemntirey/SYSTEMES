import React, { useState, useRef } from 'react'
import { useOS } from '../../context/OSContext'
import { DesktopIcon } from './DesktopIcon'
import { ContextMenu, DesktopSortMode } from './ContextMenu'
import { AppId, FileItem } from '../../types'
import { MENU_BAR_HEIGHT } from '../../data/constants'

interface DesktopProps {
  children?: React.ReactNode
}

interface DesktopShortcut {
  id: string
  name: string
  icon: string
  appId?: AppId
  fileItem?: FileItem
}

export const Desktop: React.FC<DesktopProps> = ({ children }) => {
  const {
    wallpaper,
    brightness,
    nightLight,
    openWindow,
    closeAllFlyouts,
    files,
    showDesktopLabels,
  } = useOS()

  const [contextMenuPos, setContextMenuPos] = useState<{ x: number; y: number } | null>(null)
  const [selectedIconIds, setSelectedIconIds] = useState<string[]>([])
  const [iconSize, setIconSize] = useState<'small' | 'medium' | 'large'>('medium')
  const [sortBy, setSortBy] = useState<DesktopSortMode>('name')

  // Lasso selection state
  const [selectionBox, setSelectionBox] = useState<{
    startX: number
    startY: number
    currentX: number
    currentY: number
  } | null>(null)

  const desktopRef = useRef<HTMLDivElement>(null)
  const iconRefs = useRef<Record<string, HTMLDivElement | null>>({})

  // Fixed desktop icons with accurate 3D icon names
  const defaultShortcuts: DesktopShortcut[] = [
    { id: 'pc', name: 'Ce PC', icon: 'pc', appId: 'explorer' },
    { id: 'recycle', name: 'Corbeille', icon: 'recycle' },
    { id: 'photos', name: 'Photos', icon: 'image', appId: 'photos' },
    { id: 'explorer', name: 'Explorateur', icon: 'explorer', appId: 'explorer' },
    { id: 'notepad', name: 'Bloc-notes', icon: 'notepad', appId: 'notepad' },
    { id: 'terminal', name: 'Terminal', icon: 'terminal', appId: 'terminal' },
    { id: 'calc', name: 'Calculatrice', icon: 'calculator', appId: 'calculator' },
    { id: 'paint', name: 'Paint', icon: 'paint', appId: 'paint' },
    { id: 'settings', name: 'Paramètres', icon: 'settings', appId: 'settings' },
  ]

  // Add files saved to Desktop
  const desktopFiles: DesktopShortcut[] = files
    .filter(f => f.path.startsWith('C:/Users/eemntirey/Desktop/'))
    .map(f => ({
      id: f.id,
      name: f.name,
      icon: f.type === 'folder' ? 'folder' : 'notepad',
      fileItem: f,
    }))

  const parseSize = (s?: string): number => {
    if (!s) return 0
    const m = s.replace(',', '.').match(/([\d.]+)/)
    if (!m) return 0
    const value = parseFloat(m[1])
    if (/Mo|MБ/i.test(s)) return value * 1024
    if (/Go|GB/i.test(s)) return value * 1024 * 1024
    return value
  }

  // Tri façon Windows 11 : les icônes système (Ce PC, Corbeille…) restent en tête,
  // les fichiers/dossiers du bureau sont triés selon le mode choisi.
  const sortedFiles = [...desktopFiles].sort((a, b) => {
    switch (sortBy) {
      case 'size':
        return parseSize(b.fileItem?.size) - parseSize(a.fileItem?.size)
      case 'type': {
        const ta = a.fileItem?.type ?? ''
        const tb = b.fileItem?.type ?? ''
        if (ta !== tb) return ta.localeCompare(tb)
        return a.name.localeCompare(b.name, 'fr')
      }
      case 'modified': {
        const da = a.fileItem?.modified ?? ''
        const db = b.fileItem?.modified ?? ''
        const pa = Date.parse(da)
        const pb = Date.parse(db)
        if (!Number.isNaN(pa) && !Number.isNaN(pb)) return pb - pa
        return db.localeCompare(da)
      }
      default:
        return a.name.localeCompare(b.name, 'fr')
    }
  })

  const allShortcuts = [...defaultShortcuts, ...sortedFiles]

  // Mouse handlers for selection lasso
  const handleMouseDown = (e: React.MouseEvent) => {
    if (e.target === desktopRef.current && e.button === 0) {
      closeAllFlyouts()
      setContextMenuPos(null)
      setSelectedIconIds([])
      setSelectionBox({
        startX: e.clientX,
        startY: e.clientY,
        currentX: e.clientX,
        currentY: e.clientY,
      })
    } else {
      closeAllFlyouts()
      setContextMenuPos(null)
    }
  }

  const handleMouseMove = (e: React.MouseEvent) => {
    if (selectionBox) {
      setSelectionBox(prev => (prev ? { ...prev, currentX: e.clientX, currentY: e.clientY } : null))
    }
  }

  const handleMouseUp = () => {
    if (selectionBox) {
      const left = Math.min(selectionBox.startX, selectionBox.currentX)
      const top = Math.min(selectionBox.startY, selectionBox.currentY)
      const right = Math.max(selectionBox.startX, selectionBox.currentX)
      const bottom = Math.max(selectionBox.startY, selectionBox.currentY)
      const selected: string[] = []
      for (const sc of allShortcuts) {
        const el = iconRefs.current[sc.id]
        if (el) {
          const rect = el.getBoundingClientRect()
          const cx = rect.left + rect.width / 2
          const cy = rect.top + rect.height / 2
          if (cx >= left && cx <= right && cy >= top && cy <= bottom) {
            selected.push(sc.id)
          }
        }
      }
      setSelectedIconIds(selected)
    }
    setSelectionBox(null)
  }

  // Right-click context menu
  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    closeAllFlyouts()
    setContextMenuPos({ x: e.clientX, y: e.clientY })
  }

  // Handle double clicking shortcut
  const handleShortcutDoubleClick = (sc: DesktopShortcut) => {
    if (sc.id === 'recycle') {
      openWindow('recycle')
      return
    }
    if (sc.appId) {
      openWindow(sc.appId)
    } else if (sc.fileItem) {
      if (sc.fileItem.type === 'folder') {
        openWindow('explorer', { initialPath: sc.fileItem.path })
      } else {
        openWindow(
          'notepad',
          {
            fileId: sc.fileItem.id,
            filePath: sc.fileItem.path,
            fileContent: sc.fileItem.content || '',
          },
          `${sc.fileItem.name} - Bloc-notes`
        )
      }
    }
  }

  // Calculate lasso bounds
  const getBoxStyle = () => {
    if (!selectionBox) return {}
    const left = Math.min(selectionBox.startX, selectionBox.currentX)
    const top = Math.min(selectionBox.startY, selectionBox.currentY)
    const width = Math.abs(selectionBox.currentX - selectionBox.startX)
    const height = Math.abs(selectionBox.currentY - selectionBox.startY)
    return { left, top, width, height }
  }

  return (
    <div
      className="obsidian-desktop"
      ref={desktopRef}
      onMouseDown={handleMouseDown}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onContextMenu={handleContextMenu}
      style={{
        position: 'relative',
        width: '100vw',
        height: '100vh',
        backgroundImage: `url(${wallpaper.url})`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        backgroundRepeat: 'no-repeat',
        overflow: 'hidden',
        userSelect: 'none',
      }}
    >
      {/* Night light warm filter overlay */}
      {nightLight && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            backgroundColor: 'rgba(255, 140, 0, 0.12)',
            pointerEvents: 'none',
            zIndex: 999990,
          }}
        />
      )}

      {/* Brightness adjustment overlay */}
      {brightness < 100 && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            backgroundColor: `rgba(0, 0, 0, ${(100 - brightness) / 130})`,
            pointerEvents: 'none',
            zIndex: 999991,
          }}
        />
      )}

      {/* Selection Lasso Box */}
      {selectionBox && <div className="selection-box" style={getBoxStyle()} />}

      {/* Desktop Icons Grid — minimal macOS style */}
      <div
        className="desktop-icons-grid"
        style={{
          position: 'absolute',
          top: `${MENU_BAR_HEIGHT + 20}px`,
          left: '20px',
          display: 'flex',
          flexWrap: 'wrap',
          alignContent: 'flex-start',
          gap: '8px 12px',
          zIndex: 5,
          pointerEvents: 'none',
        }}
      >
        {allShortcuts.map(sc => (
          <div key={sc.id} style={{ pointerEvents: 'all' }}>
            <DesktopIcon
              ref={(el) => { iconRefs.current[sc.id] = el }}
              id={sc.id}
              name={sc.name}
              icon={sc.icon}
              isSelected={selectedIconIds.includes(sc.id) || false}
              showLabel={showDesktopLabels}
              iconSize={iconSize}
              onClick={e => {
                e.stopPropagation()
                if (e.ctrlKey || e.metaKey) {
                  setSelectedIconIds(prev => prev.includes(sc.id) ? prev.filter(id => id !== sc.id) : [...prev, sc.id])
                } else {
                  setSelectedIconIds([sc.id])
                }
              }}
              onDoubleClick={e => {
                e.stopPropagation()
                handleShortcutDoubleClick(sc)
              }}
            />
          </div>
        ))}
      </div>

      {/* Render Active Windows */}
      {children}

      {/* Right Click Context Menu */}
      {contextMenuPos && (
        <ContextMenu
          x={contextMenuPos.x}
          y={contextMenuPos.y}
          onClose={() => setContextMenuPos(null)}
          iconSize={iconSize}
          onIconSizeChange={setIconSize}
          sortBy={sortBy}
          onSortChange={setSortBy}
        />
      )}
    </div>
  )
}
