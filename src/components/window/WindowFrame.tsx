import React, { useState, useRef, useEffect, ReactNode, useCallback } from 'react'
import { SnapPosition, WindowState } from '../../types'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { useDisplayLayout } from '../../hooks/useDisplayLayout'
import {
  clampPosition,
  maximizeRect,
  monitorAt,
  snapRect,
  toCssRect,
} from '../../utils/windowPlacement'

interface WindowFrameProps {
  window: WindowState
  children: ReactNode
}

const MIN_WIDTH = 340
const MIN_HEIGHT = 260
const SNAP_MARGIN = 28

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

  const layout = useDisplayLayout()
  const isActive = activeWindowId === win.id
  const [showSnapMenu, setShowSnapMenu] = useState(false)
  const snapTimeoutRef = useRef<number | null>(null)

  const isDraggingRef = useRef(false)
  const isResizingRef = useRef(false)
  const resizeDirectionRef = useRef<string | null>(null)

  const livePosRef = useRef({ x: win.position.x, y: win.position.y })
  const liveSizeRef = useRef({ width: win.size.width, height: win.size.height })

  const dragStartRef = useRef({ x: 0, y: 0, winX: 0, winY: 0 })
  const resizeStartRef = useRef({
    x: 0, y: 0, winX: 0, winY: 0, winW: 0, winH: 0,
  })

  useEffect(() => {
    if (!isDraggingRef.current && !isResizingRef.current) {
      livePosRef.current = { x: win.position.x, y: win.position.y }
      liveSizeRef.current = { width: win.size.width, height: win.size.height }
    }
  }, [win.position.x, win.position.y, win.size.width, win.size.height])

  const handleTitleBarMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return
    if (win.isMaximized) return
    if ((e.target as HTMLElement).closest('button')) return

    bringToFront(win.id)
    isDraggingRef.current = true
    dragStartRef.current = {
      x: e.clientX,
      y: e.clientY,
      winX: win.position.x,
      winY: win.position.y,
    }
    livePosRef.current = { x: win.position.x, y: win.position.y }
  }

  const handleResizeMouseDown = (e: React.MouseEvent, direction: string) => {
    e.stopPropagation()
    e.preventDefault()
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
    livePosRef.current = { x: win.position.x, y: win.position.y }
    liveSizeRef.current = { width: win.size.width, height: win.size.height }
  }

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (isDraggingRef.current) {
      const dx = e.clientX - dragStartRef.current.x
      const dy = e.clientY - dragStartRef.current.y
      const rawX = dragStartRef.current.winX + dx
      const rawY = dragStartRef.current.winY + dy
      const clamped = clampPosition(
        rawX,
        rawY,
        liveSizeRef.current.width,
        liveSizeRef.current.height,
        layout,
      )
      livePosRef.current = clamped
      updateWindowPosition(win.id, clamped.x, clamped.y)
    } else if (isResizingRef.current && resizeDirectionRef.current) {
      const dx = e.clientX - resizeStartRef.current.x
      const dy = e.clientY - resizeStartRef.current.y
      const dir = resizeDirectionRef.current

      let newW = resizeStartRef.current.winW
      let newH = resizeStartRef.current.winH
      let newX = resizeStartRef.current.winX
      let newY = resizeStartRef.current.winY

      if (dir.includes('e')) newW = Math.max(MIN_WIDTH, resizeStartRef.current.winW + dx)
      if (dir.includes('s')) newH = Math.max(MIN_HEIGHT, resizeStartRef.current.winH + dy)
      if (dir.includes('w')) {
        const potentialW = resizeStartRef.current.winW - dx
        if (potentialW >= MIN_WIDTH) {
          newW = potentialW
          newX = resizeStartRef.current.winX + dx
        }
      }
      if (dir.includes('n')) {
        const potentialH = resizeStartRef.current.winH - dy
        if (potentialH >= MIN_HEIGHT) {
          newH = potentialH
          newY = resizeStartRef.current.winY + dy
        }
      }

      const mon = monitorAt(newX + newW / 2, newY + 20, layout)
      const wa = mon.workArea
      if (newX < wa.x) {
        newW -= wa.x - newX
        newX = wa.x
      }
      if (newY < wa.y) {
        newH -= wa.y - newY
        newY = wa.y
      }
      if (newX + newW > wa.x + wa.width) newW = Math.max(MIN_WIDTH, wa.x + wa.width - newX)
      if (newY + newH > wa.y + wa.height) newH = Math.max(MIN_HEIGHT, wa.y + wa.height - newY)

      livePosRef.current = { x: newX, y: newY }
      liveSizeRef.current = { width: newW, height: newH }
      updateWindowPosition(win.id, newX, newY)
      updateWindowSize(win.id, newW, newH)
    }
  }, [win.id, updateWindowPosition, updateWindowSize, layout])

  const handleMouseUp = useCallback(() => {
    const wasDragging = isDraggingRef.current
    const wasResizing = isResizingRef.current

    isDraggingRef.current = false
    isResizingRef.current = false
    resizeDirectionRef.current = null

    if (!wasDragging || wasResizing || win.isMaximized) return

    const x = livePosRef.current.x
    const y = livePosRef.current.y
    const w = liveSizeRef.current.width
    const mon = monitorAt(x + w / 2, y + 10, layout)
    const wa = mon.workArea

    if (x <= wa.x + SNAP_MARGIN) {
      snapWindow(win.id, 'left-half')
    } else if (x + w >= wa.x + wa.width - SNAP_MARGIN) {
      snapWindow(win.id, 'right-half')
    } else if (y <= wa.y + SNAP_MARGIN) {
      maximizeWindow(win.id)
    }
  }, [win.id, snapWindow, maximizeWindow, win.isMaximized, layout])

  useEffect(() => {
    window.addEventListener('mousemove', handleMouseMove)
    window.addEventListener('mouseup', handleMouseUp)
    return () => {
      window.removeEventListener('mousemove', handleMouseMove)
      window.removeEventListener('mouseup', handleMouseUp)
    }
  }, [handleMouseMove, handleMouseUp])

  useEffect(() => {
    return () => {
      if (snapTimeoutRef.current) clearTimeout(snapTimeoutRef.current)
    }
  }, [])

  if (win.isMinimized) return null

  const css = win.isMaximized
    ? (() => {
        const mon = monitorAt(win.position.x + 40, win.position.y + 20, layout)
        const r = maximizeRect(mon)
        return toCssRect(r, layout)
      })()
    : toCssRect(
        { x: win.position.x, y: win.position.y, width: win.size.width, height: win.size.height },
        layout,
      )

  const frameStyle: React.CSSProperties = {
    position: 'absolute',
    left: `${css.x}px`,
    top: `${css.y}px`,
    width: `${css.width}px`,
    height: `${css.height}px`,
    zIndex: win.zIndex,
    borderRadius: win.isMaximized ? 0 : 'var(--radius-xl)',
    boxShadow: win.isMaximized
      ? 'none'
      : isActive
        ? 'var(--window-shadow), 0 0 0 1px var(--window-border)'
        : '0 16px 36px rgba(0,0,0,0.35), 0 0 0 1px rgba(139,92,246,0.15)',
    border: win.isMaximized
      ? 'none'
      : isActive
        ? '1px solid var(--window-border)'
        : '1px solid rgba(139,92,246,0.2)',
    backgroundColor: 'var(--window-bg)',
    backdropFilter: 'var(--backdrop-blur)',
    WebkitBackdropFilter: 'var(--backdrop-blur)',
    display: 'flex',
    flexDirection: 'column',
    overflow: 'hidden',
    transition: win.isMaximized ? 'none' : 'box-shadow 0.15s ease, border-color 0.15s ease',
    animation: win.isMaximized ? undefined : 'windowOpen 0.2s cubic-bezier(0.16, 1, 0.3, 1)',
  }

  const applySnap = (pos: SnapPosition) => {
    const mon = monitorAt(
      livePosRef.current.x + liveSizeRef.current.width / 2,
      livePosRef.current.y + 10,
      layout,
    )
    const r = snapRect(pos, mon)
    updateWindowPosition(win.id, r.x, r.y)
    updateWindowSize(win.id, r.width, r.height)
    snapWindow(win.id, pos)
    setShowSnapMenu(false)
  }

  return (
    <div onMouseDown={() => bringToFront(win.id)} style={frameStyle}>
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
          flexShrink: 0,
          userSelect: 'none',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '10px', overflow: 'hidden', minWidth: 0 }}>
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

        <div style={{ display: 'flex', height: '100%', alignItems: 'stretch', flexShrink: 0 }}>
          <button
            type="button"
            onClick={(e) => { e.stopPropagation(); minimizeWindow(win.id) }}
            title="Réduire"
            style={{
              width: '46px', border: 'none', background: 'transparent', color: 'var(--text-primary)',
              cursor: 'default', display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'background 0.1s',
            }}
            onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.09)' }}
            onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'transparent' }}
          >
            <svg width="10" height="10" viewBox="0 0 10 10"><line x1="0" y1="5" x2="10" y2="5" stroke="currentColor" strokeWidth="1" /></svg>
          </button>

          <div
            style={{ position: 'relative', display: 'flex' }}
            onMouseEnter={() => {
              snapTimeoutRef.current = window.setTimeout(() => setShowSnapMenu(true), 320)
            }}
            onMouseLeave={() => {
              if (snapTimeoutRef.current) clearTimeout(snapTimeoutRef.current)
              setShowSnapMenu(false)
            }}
          >
            <button
              type="button"
              onClick={(e) => { e.stopPropagation(); maximizeWindow(win.id) }}
              title={win.isMaximized ? 'Niveau inférieur' : 'Agrandir'}
              style={{
                width: '46px', border: 'none', background: 'transparent', color: 'var(--text-primary)',
                cursor: 'default', display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'background 0.1s',
              }}
              onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.09)' }}
              onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'transparent' }}
            >
              {win.isMaximized ? (
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1"><rect x="0.5" y="2.5" width="7" height="7" rx="1" /><path d="M2.5 0.5h6a1 1 0 011 1v6" /></svg>
              ) : (
                <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1"><rect x="0.5" y="0.5" width="9" height="9" rx="1.5" /></svg>
              )}
            </button>

            {showSnapMenu && (
              <div
                style={{
                  position: 'absolute', top: '38px', right: '0px', width: '240px',
                  backgroundColor: 'var(--flyout-bg)', backdropFilter: 'var(--backdrop-blur)',
                  border: '1px solid var(--flyout-border)', borderRadius: 'var(--radius-md)',
                  padding: '12px', boxShadow: '0 12px 30px rgba(0,0,0,0.6), 0 0 20px rgba(139, 92, 246, 0.3)',
                  zIndex: 9999, display: 'grid', gridTemplateColumns: 'repeat(2, 1fr)', gap: '8px',
                }}
              >
                <div style={{ display: 'flex', gap: '4px', height: '56px', padding: '4px', borderRadius: '8px', backgroundColor: 'rgba(139, 92, 246, 0.08)' }}>
                  <div
                    onClick={() => applySnap('left-half')}
                    title="Ancrer à gauche"
                    style={{ flex: 1, backgroundColor: 'rgba(255,255,255,0.1)', borderRadius: '4px', cursor: 'pointer', transition: 'all 0.15s' }}
                    onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--win-accent-cyan)' }}
                    onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)' }}
                  />
                  <div
                    onClick={() => applySnap('right-half')}
                    title="Ancrer à droite"
                    style={{ flex: 1, backgroundColor: 'rgba(255,255,255,0.1)', borderRadius: '4px', cursor: 'pointer', transition: 'all 0.15s' }}
                    onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--win-accent-cyan)' }}
                    onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)' }}
                  />
                </div>

                <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gridTemplateRows: '1fr 1fr', gap: '4px', height: '56px', padding: '4px', borderRadius: '8px', backgroundColor: 'rgba(139, 92, 246, 0.08)' }}>
                  {(['top-left', 'top-right', 'bottom-left', 'bottom-right'] as SnapPosition[]).map(pos => (
                    <div
                      key={pos}
                      onClick={() => applySnap(pos)}
                      title={`Ancrer ${pos}`}
                      style={{ backgroundColor: 'rgba(255,255,255,0.1)', borderRadius: '3px', cursor: 'pointer', transition: 'all 0.15s' }}
                      onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--win-accent-cyan)' }}
                      onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'rgba(255,255,255,0.1)' }}
                    />
                  ))}
                </div>
              </div>
            )}
          </div>

          <button
            type="button"
            onClick={(e) => { e.stopPropagation(); closeWindow(win.id) }}
            title="Fermer"
            style={{
              width: '46px', border: 'none', background: 'transparent', color: 'var(--text-primary)',
              cursor: 'default', display: 'flex', alignItems: 'center', justifyContent: 'center', transition: 'background 0.1s',
            }}
            onMouseEnter={e => {
              e.currentTarget.style.backgroundColor = '#c42b1c'
              e.currentTarget.style.color = '#ffffff'
            }}
            onMouseLeave={e => {
              e.currentTarget.style.backgroundColor = 'transparent'
              e.currentTarget.style.color = 'var(--text-primary)'
            }}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" stroke="currentColor" strokeWidth="1"><line x1="0" y1="0" x2="10" y2="10" /><line x1="10" y1="0" x2="0" y2="10" /></svg>
          </button>
        </div>
      </div>

      <div style={{ flex: 1, overflow: 'auto', position: 'relative', minHeight: 0 }}>{children}</div>

      {!win.isMaximized && (
        <>
          <div onMouseDown={e => handleResizeMouseDown(e, 'n')} style={{ position: 'absolute', top: 0, left: 10, right: 10, height: 6, cursor: 'n-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 's')} style={{ position: 'absolute', bottom: 0, left: 10, right: 10, height: 6, cursor: 's-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 'w')} style={{ position: 'absolute', left: 0, top: 10, bottom: 10, width: 6, cursor: 'w-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 'e')} style={{ position: 'absolute', right: 0, top: 10, bottom: 10, width: 6, cursor: 'e-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 'nw')} style={{ position: 'absolute', top: 0, left: 0, width: 12, height: 12, cursor: 'nw-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 'ne')} style={{ position: 'absolute', top: 0, right: 0, width: 12, height: 12, cursor: 'ne-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 'sw')} style={{ position: 'absolute', bottom: 0, left: 0, width: 12, height: 12, cursor: 'sw-resize' }} />
          <div onMouseDown={e => handleResizeMouseDown(e, 'se')} style={{ position: 'absolute', bottom: 0, right: 0, width: 12, height: 12, cursor: 'se-resize' }} />
        </>
      )}
    </div>
  )
}
