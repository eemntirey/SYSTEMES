import React, { useState, useRef, useEffect, ReactNode, useCallback } from 'react'
import { SnapPosition, WindowState } from '../../types'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { MENU_BAR_HEIGHT } from '../../data/constants'

interface WindowFrameProps {
  window: WindowState
  children: ReactNode
}

export const WindowFrame: React.FC<WindowFrameProps> = ({ window: win, children }) => {
  const {
    activeWindowId,
    bringToFront,
    closeWindow,
    minimizeWindow,
    maximizeWindow,
    updateWindowPosition,
    updateWindowSize,
    snapWindow,
  } = useOS()

  const isActive = activeWindowId === win.id
  const [showSnapMenu, setShowSnapMenu] = useState(false)
  const snapTimeoutRef = useRef<number | null>(null)

  // Dragging state
  const isDraggingRef = useRef(false)
  const dragStartRef = useRef<{ x: number; y: number; winX: number; winY: number }>({
    x: 0,
    y: 0,
    winX: win.position.x,
    winY: win.position.y,
  })

  // Resizing state
  const isResizingRef = useRef(false)
  const resizeDirectionRef = useRef<string | null>(null)
  const resizeStartRef = useRef<{
    x: number
    y: number
    winX: number
    winY: number
    winW: number
    winH: number
  }>({
    x: 0,
    y: 0,
    winX: win.position.x,
    winY: win.position.y,
    winW: win.size.width,
    winH: win.size.height,
  })

  // Drag handlers
  const handleTitleBarMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return
    if (win.isMaximized) return

    bringToFront(win.id)
    isDraggingRef.current = true
    dragStartRef.current = {
      x: e.clientX,
      y: e.clientY,
      winX: win.position.x,
      winY: win.position.y,
    }
  }

  // Resize handler
  const handleResizeMouseDown = (e: React.MouseEvent, direction: string) => {
    e.stopPropagation()
    if (win.isMaximized) return

    bringToFront(win.id)
    isResizingRef.current = true
    resizeDirectionRef.current = direction
    resizeStartRef.current = {
      x: e.clientX,
      y: e.clientY,
      winX: win.position.x,
      winY: win.position.y,
      winW: win.size.width,
      winH: win.size.height,
    }
  }

  // Global mouse listeners for drag & resize
  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (isDraggingRef.current) {
      const dx = e.clientX - dragStartRef.current.x
      const dy = e.clientY - dragStartRef.current.y
      const newX = Math.max(0, Math.min(window.innerWidth - 100, dragStartRef.current.winX + dx))
      const newY = Math.max(MENU_BAR_HEIGHT, Math.min(window.innerHeight - 80, dragStartRef.current.winY + dy))
      updateWindowPosition(win.id, newX, newY)
    } else if (isResizingRef.current && resizeDirectionRef.current) {
      const dx = e.clientX - resizeStartRef.current.x
      const dy = e.clientY - resizeStartRef.current.y
      const dir = resizeDirectionRef.current

      let newW = resizeStartRef.current.winW
      let newH = resizeStartRef.current.winH
      let newX = resizeStartRef.current.winX
      let newY = resizeStartRef.current.winY

      if (dir.includes('e')) newW = Math.max(340, resizeStartRef.current.winW + dx)
      if (dir.includes('s')) newH = Math.max(260, resizeStartRef.current.winH + dy)
      if (dir.includes('w')) {
        const potentialW = resizeStartRef.current.winW - dx
        if (potentialW >= 340) {
          newW = potentialW
          newX = resizeStartRef.current.winX + dx
        }
      }
      if (dir.includes('n')) {
        const potentialH = resizeStartRef.current.winH - dy
        if (potentialH >= 260) {
          newH = potentialH
          newY = resizeStartRef.current.winY + dy
        }
      }

      updateWindowPosition(win.id, newX, newY)
      updateWindowSize(win.id, newW, newH)
    }
  }, [win.id, updateWindowPosition, updateWindowSize])

  const handleMouseUp = useCallback(() => {
    isDraggingRef.current = false
    isResizingRef.current = false
    resizeDirectionRef.current = null
  }, [])

  useEffect(() => {
    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [handleMouseMove, handleMouseUp])

  if (win.isMinimized) return null

  return (
    <div
      onMouseDown={() => bringToFront(win.id)}
      style={{
        position: 'absolute',
        left: `${win.position.x}px`,
        top: `${win.position.y}px`,
        width: `${win.size.width}px`,
        height: `${win.size.height}px`,
        zIndex: win.zIndex,
        borderRadius: win.isMaximized ? '0px' : 'var(--radius-xl)',
        boxShadow: isActive
          ? 'var(--window-shadow), 0 0 0 1px var(--window-border)'
          : '0 16px 36px rgba(0,0,0,0.35), 0 0 0 1px rgba(139,92,246,0.15)',
        border: isActive
          ? '1px solid var(--window-border)'
          : '1px solid rgba(139,92,246,0.2)',
        backgroundColor: 'var(--window-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        transition: win.isMaximized ? 'all 0.18s ease-out' : 'box-shadow 0.15s ease, border-color 0.15s ease',
        animation: 'windowOpen 0.2s cubic-bezier(0.16, 1, 0.3, 1)',
      }}
    >
      {/* Title Bar */}
      <div
        onMouseDown={handleTitleBarMouseDown}
        onDoubleClick={() => maximizeWindow(win.id)}
        style={{
          height: '42px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 0 0 16px',
          backgroundColor: 'var(--window-titlebar)',
          borderBottom: '1px solid rgba(139,92,246,0.15)',
          cursor: win.isMaximized ? 'default' : 'move',
          position: 'relative',
        }}
      >
        {/* App Title & Icon */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', overflow: 'hidden' }}>
          <FluentIcon name={win.icon} size={17} color="var(--win-accent-cyan)" />
          <span
            style={{
              fontSize: '13px',
              fontWeight: 600,
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
            }}
          >
            {win.title}
          </span>
        </div>

        {/* Window Controls */}
        <div style={{ display: 'flex', height: '100%', alignItems: 'center', paddingRight: '6px', gap: '4px' }}>
          {/* Minimize */}
          <button
            onClick={() => minimizeWindow(win.id)}
            title="Réduire"
            style={{
              width: '32px',
              height: '32px',
              borderRadius: '50%',
              border: 'none',
              background: 'transparent',
              color: 'var(--text-secondary)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              transition: 'background 0.15s, color 0.15s',
            }}
            onMouseEnter={e => {
              e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.15)'
              e.currentTarget.style.color = '#ffffff'
            }}
            onMouseLeave={e => {
              e.currentTarget.style.backgroundColor = 'transparent'
              e.currentTarget.style.color = 'var(--text-secondary)'
            }}
          >
            <FluentIcon name="minus" size={13} />
          </button>

          {/* Maximize / Restore with Snap Menu Trigger */}
          <div
            style={{ position: 'relative', height: '32px', display: 'flex', alignItems: 'center' }}
            onMouseEnter={() => {
              snapTimeoutRef.current = window.setTimeout(() => setShowSnapMenu(true), 300)
            }}
            onMouseLeave={() => {
              if (snapTimeoutRef.current) clearTimeout(snapTimeoutRef.current)
              setShowSnapMenu(false)
            }}
          >
            <button
              onClick={() => maximizeWindow(win.id)}
              title={win.isMaximized ? 'Niveau inférieur' : 'Agrandir'}
              style={{
                width: '32px',
                height: '32px',
                borderRadius: '50%',
                border: 'none',
                background: 'transparent',
                color: 'var(--text-secondary)',
                cursor: 'pointer',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                transition: 'background 0.15s, color 0.15s',
              }}
              onMouseEnter={e => {
                e.currentTarget.style.backgroundColor = 'var(--control-hover)'
                e.currentTarget.style.color = '#ffffff'
              }}
              onMouseLeave={e => {
                e.currentTarget.style.backgroundColor = 'transparent'
                e.currentTarget.style.color = 'var(--text-secondary)'
              }}
            >
              <FluentIcon name={win.isMaximized ? 'minimize' : 'square'} size={12} />
            </button>

            {/* Windows 11 Snap Layouts flyout */}
            {showSnapMenu && (
              <div
                style={{
                  position: 'absolute',
                  top: '38px',
                  right: '0px',
                  width: '240px',
                  backgroundColor: 'var(--flyout-bg)',
                  backdropFilter: 'var(--backdrop-blur)',
                  border: '1px solid var(--flyout-border)',
                  borderRadius: 'var(--radius-md)',
                  padding: '12px',
                  boxShadow: '0 12px 30px rgba(0,0,0,0.6), 0 0 20px rgba(139, 92, 246, 0.3)',
                  zIndex: 9999,
                  display: 'grid',
                  gridTemplateColumns: 'repeat(2, 1fr)',
                  gap: '8px',
                }}
              >
                {/* 50 / 50 Layout */}
                <div
                  style={{
                    display: 'flex',
                    gap: '4px',
                    height: '56px',
                    padding: '4px',
                    borderRadius: '8px',
                    backgroundColor: 'rgba(139, 92, 246, 0.08)',
                    cursor: 'pointer',
                  }}
                >
                  <div
                    onClick={() => {
                      snapWindow(win.id, 'left-half')
                      setShowSnapMenu(false)
                    }}
                    title="Ancrer à gauche"
                    style={{
                      flex: 1,
                      backgroundColor: 'rgba(255,255,255,0.1)',
                      borderRadius: '4px',
                      transition: 'all 0.15s',
                    }}
                    onMouseEnter={e => {
                      e.currentTarget.style.backgroundColor = 'var(--win-accent-cyan)'
                      e.currentTarget.style.boxShadow = '0 0 8px var(--win-accent-cyan)'
                    }}
                    onMouseLeave={e => {
                      e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'
                      e.currentTarget.style.boxShadow = 'none'
                    }}
                  />
                  <div
                    onClick={() => {
                      snapWindow(win.id, 'right-half')
                      setShowSnapMenu(false)
                    }}
                    title="Ancrer à droite"
                    style={{
                      flex: 1,
                      backgroundColor: 'rgba(255,255,255,0.1)',
                      borderRadius: '4px',
                      transition: 'all 0.15s',
                    }}
                    onMouseEnter={e => {
                      e.currentTarget.style.backgroundColor = 'var(--win-accent-cyan)'
                      e.currentTarget.style.boxShadow = '0 0 8px var(--win-accent-cyan)'
                    }}
                    onMouseLeave={e => {
                      e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'
                      e.currentTarget.style.boxShadow = 'none'
                    }}
                  />
                </div>

                {/* 4 Quadrants Layout */}
                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: '1fr 1fr',
                    gridTemplateRows: '1fr 1fr',
                    gap: '4px',
                    height: '56px',
                    padding: '4px',
                    borderRadius: '8px',
                    backgroundColor: 'rgba(139, 92, 246, 0.08)',
                    cursor: 'pointer',
                  }}
                >
                  {(['top-left', 'top-right', 'bottom-left', 'bottom-right'] as SnapPosition[]).map(pos => (
                    <div
                      key={pos}
                      onClick={() => {
                        snapWindow(win.id, pos)
                        setShowSnapMenu(false)
                      }}
                      title={`Ancrer ${pos}`}
                      style={{
                        backgroundColor: 'rgba(255,255,255,0.1)',
                        borderRadius: '3px',
                        transition: 'all 0.15s',
                      }}
                      onMouseEnter={e => {
                        e.currentTarget.style.backgroundColor = 'var(--win-accent-cyan)'
                        e.currentTarget.style.boxShadow = '0 0 8px var(--win-accent-cyan)'
                      }}
                      onMouseLeave={e => {
                        e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)'
                        e.currentTarget.style.boxShadow = 'none'
                      }}
                    />
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Close */}
          <button
            onClick={() => closeWindow(win.id)}
            title="Fermer"
            style={{
              width: '32px',
              height: '32px',
              borderRadius: '50%',
              border: 'none',
              background: 'transparent',
              color: 'var(--text-secondary)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              transition: 'all 0.15s',
            }}
            onMouseEnter={e => {
              e.currentTarget.style.backgroundColor = 'rgba(255, 80, 80, 0.2)'
              e.currentTarget.style.color = '#ff5555'
            }}
            onMouseLeave={e => {
              e.currentTarget.style.backgroundColor = 'transparent'
              e.currentTarget.style.color = 'var(--text-secondary)'
            }}
          >
            <FluentIcon name="x" size={14} />
          </button>
        </div>
      </div>

      {/* Window Body */}
      <div style={{ flex: 1, overflow: 'auto', position: 'relative' }}>{children}</div>

      {/* Resize handles (disabled if maximized) */}
      {!win.isMaximized && (
        <>
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'n')}
            style={{ position: 'absolute', top: 0, left: 10, right: 10, height: 6, cursor: 'n-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 's')}
            style={{ position: 'absolute', bottom: 0, left: 10, right: 10, height: 6, cursor: 's-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'w')}
            style={{ position: 'absolute', left: 0, top: 10, bottom: 10, width: 6, cursor: 'w-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'e')}
            style={{ position: 'absolute', right: 0, top: 10, bottom: 10, width: 6, cursor: 'e-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'nw')}
            style={{ position: 'absolute', top: 0, left: 0, width: 12, height: 12, cursor: 'nw-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'ne')}
            style={{ position: 'absolute', top: 0, right: 0, width: 12, height: 12, cursor: 'ne-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'sw')}
            style={{ position: 'absolute', bottom: 0, left: 0, width: 12, height: 12, cursor: 'sw-resize' }}
          />
          <div
            onMouseDown={e => handleResizeMouseDown(e, 'se')}
            style={{ position: 'absolute', bottom: 0, right: 0, width: 12, height: 12, cursor: 'se-resize' }}
          />
        </>
      )}
    </div>
  )
}
