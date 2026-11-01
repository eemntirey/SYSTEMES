import React from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon, WindowsLogo } from '../common/FluentIcon'
import { TASKBAR_HEIGHT } from '../../data/constants'

/**
 * Windows 11 Task View (Vue des tâches).
 * - Fond flouté/acrylique sombre.
 * - Vignettes des fenêtres ouvertes (icône + titre + bouton fermer au survol).
 * - Barre des bureaux virtuels en bas : « Bureau 1 » + « Nouveau bureau ».
 * - Échap ou clic hors zone ferme la vue.
 */
export const TaskView: React.FC = () => {
  const {
    isTaskViewOpen,
    closeAllFlyouts,
    windows,
    bringToFront,
    closeWindow,
    openWindow,
    activeWindowId,
  } = useOS()

  if (!isTaskViewOpen) return null

  const handleClose = () => closeAllFlyouts()

  const handleSelect = (id: string) => {
    bringToFront(id)
    closeAllFlyouts()
  }

  return (
    <div
      onClick={handleClose}
      style={{
        position: 'fixed',
        inset: 0,
        bottom: TASKBAR_HEIGHT,
        zIndex: 999950,
        background: 'rgba(10, 10, 18, 0.72)',
        backdropFilter: 'blur(42px) saturate(150%)',
        WebkitBackdropFilter: 'blur(42px) saturate(150%)',
        display: 'flex',
        flexDirection: 'column',
        animation: 'flyoutOpen 0.18s ease-out',
        fontFamily: "'Inter', 'Segoe UI', system-ui, sans-serif",
      }}
    >
      {/* Zone des vignettes */}
      <div
        onClick={e => e.stopPropagation()}
        style={{
          flex: 1,
          display: 'flex',
          flexWrap: 'wrap',
          alignContent: 'center',
          justifyContent: 'center',
          gap: '20px',
          padding: '40px 60px 20px',
          overflow: 'auto',
        }}
      >
        {windows.length === 0 && (
          <div
            style={{
              alignSelf: 'center',
              color: 'var(--text-secondary)',
              fontSize: 14,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              gap: '14px',
            }}
          >
            <WindowsLogo size={54} />
            <span>Aucune fenêtre ouverte</span>
            <button
              onClick={() => {
                openWindow('explorer')
                closeAllFlyouts()
              }}
              style={{
                marginTop: 4,
                padding: '8px 18px',
                borderRadius: '6px',
                border: '1px solid var(--flyout-border)',
                background: 'var(--taskbar-item-hover)',
                color: 'var(--text-primary)',
                fontSize: 13,
                cursor: 'pointer',
              }}
            >
              Ouvrir l’Explorateur
            </button>
          </div>
        )}

        {windows.map(win => {
          const isActive = win.id === activeWindowId && !win.isMinimized
          return (
            <div
              key={win.id}
              onClick={() => handleSelect(win.id)}
              style={{
                width: '260px',
                cursor: 'pointer',
                display: 'flex',
                flexDirection: 'column',
                gap: '8px',
              }}
            >
              {/* Vignette */}
              <div
                style={{
                  position: 'relative',
                  width: '100%',
                  height: '160px',
                  borderRadius: '8px',
                  border: `1px solid ${isActive ? 'var(--win-accent)' : 'var(--flyout-border)'}`,
                  background: 'var(--window-bg)',
                  boxShadow: isActive
                    ? '0 0 0 2px var(--win-accent), 0 18px 40px rgba(0,0,0,0.55)'
                    : '0 12px 30px rgba(0,0,0,0.45)',
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: '12px',
                  transition: 'transform 0.14s ease, box-shadow 0.14s ease',
                  overflow: 'hidden',
                }}
                onMouseEnter={e => {
                  e.currentTarget.style.transform = 'translateY(-4px) scale(1.02)'
                }}
                onMouseLeave={e => {
                  e.currentTarget.style.transform = 'none'
                }}
              >
                {/* Mini barre de titre factice */}
                <div
                  style={{
                    position: 'absolute',
                    top: 0,
                    left: 0,
                    right: 0,
                    height: '26px',
                    display: 'flex',
                    alignItems: 'center',
                    gap: '6px',
                    padding: '0 8px',
                    background: 'var(--window-titlebar)',
                    borderBottom: '1px solid var(--window-border)',
                  }}
                >
                  <FluentIcon name={win.icon} size={12} color="var(--win-accent-cyan)" />
                  <span
                    style={{
                      fontSize: 11,
                      color: 'var(--text-secondary)',
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                    }}
                  >
                    {win.title}
                  </span>
                </div>

                <FluentIcon name={win.icon} size={42} color="var(--win-accent)" />

                {/* Bouton fermer */}
                <button
                  onClick={e => {
                    e.stopPropagation()
                    closeWindow(win.id)
                  }}
                  aria-label={`Fermer ${win.title}`}
                  title="Fermer"
                  style={{
                    position: 'absolute',
                    top: '32px',
                    right: '6px',
                    width: '22px',
                    height: '22px',
                    borderRadius: '4px',
                    border: 'none',
                    background: 'rgba(0,0,0,0.55)',
                    color: '#fff',
                    cursor: 'pointer',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transition: 'background 0.12s',
                  }}
                  onMouseEnter={e => {
                    e.currentTarget.style.backgroundColor = '#c42b1c'
                  }}
                  onMouseLeave={e => {
                    e.currentTarget.style.backgroundColor = 'rgba(0,0,0,0.55)'
                  }}
                >
                  <FluentIcon name="x" size={12} />
                </button>
              </div>

              {/* Légende */}
              <div
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  gap: '6px',
                  fontSize: 12.5,
                  color: isActive ? 'var(--win-accent)' : 'var(--text-primary)',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                <FluentIcon name={win.icon} size={13} />
                <span style={{ overflow: 'hidden', textOverflow: 'ellipsis' }}>{win.title}</span>
              </div>
            </div>
          )
        })}
      </div>

      {/* Barre des bureaux virtuels */}
      <div
        onClick={e => e.stopPropagation()}
        style={{
          height: '104px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: '12px',
          borderTop: '1px solid rgba(255,255,255,0.06)',
          padding: '0 24px',
        }}
      >
        {/* Bureau actuel */}
        <div
          style={{
            width: '150px',
            height: '78px',
            borderRadius: '6px',
            border: '2px solid var(--win-accent)',
            background: 'var(--window-bg)',
            display: 'flex',
            alignItems: 'flex-end',
            padding: '6px',
            boxShadow: '0 0 0 1px rgba(0,0,0,0.4)',
          }}
        >
          <span style={{ fontSize: 12, color: 'var(--text-primary)' }}>Bureau 1</span>
        </div>

        {/* Nouveau bureau */}
        <button
          onClick={() => {
            // Les bureaux virtuels natifs arrivent dans le bloc dédié
          }}
          title="Nouveau bureau"
          style={{
            width: '150px',
            height: '78px',
            borderRadius: '6px',
            border: '1px dashed var(--flyout-border)',
            background: 'transparent',
            color: 'var(--text-secondary)',
            cursor: 'pointer',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            gap: '4px',
            fontSize: 12,
            transition: 'background 0.12s',
          }}
          onMouseEnter={e => {
            e.currentTarget.style.backgroundColor = 'var(--taskbar-item-hover)'
          }}
          onMouseLeave={e => {
            e.currentTarget.style.backgroundColor = 'transparent'
          }}
        >
          <FluentIcon name="plus" size={18} />
          <span>Nouveau bureau</span>
        </button>
      </div>
    </div>
  )
}
