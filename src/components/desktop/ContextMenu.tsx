import React from 'react'
import { FluentIcon } from '../common/FluentIcon'
import { useOS } from '../../context/OSContext'

interface ContextMenuProps {
  x: number
  y: number
  onClose: () => void
  iconSize?: 'small' | 'medium' | 'large'
  onIconSizeChange?: (size: 'small' | 'medium' | 'large') => void
}

export const ContextMenu: React.FC<ContextMenuProps> = ({ x, y, onClose, iconSize = 'medium', onIconSizeChange }) => {
  const { createFolder, createFile, openWindow, addNotification, showDesktopLabels, toggleDesktopLabels } = useOS()

  // Ensure menu stays within screen boundaries
  const menuWidth = 240
  const menuHeight = 280
  const adjustedX = Math.min(x, window.innerWidth - menuWidth - 10)
  const adjustedY = Math.min(y, window.innerHeight - menuHeight - 50)

  return (
    <div
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
      style={{
        position: 'fixed',
        left: `${adjustedX}px`,
        top: `${adjustedY}px`,
        width: `${menuWidth}px`,
        backgroundColor: 'var(--flyout-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: '8px',
        padding: '6px',
        boxShadow: '0 12px 32px rgba(0,0,0,0.5)',
        zIndex: 99999,
        pointerEvents: 'auto',
        fontSize: '12.5px',
        display: 'flex',
        flexDirection: 'column',
        gap: '2px',
        animation: 'flyoutOpen 0.14s ease-out',
      }}
    >
      {/* Actualiser */}
      <div
        onClick={() => {
          addNotification('Bureau', 'Bureau actualisé avec succès', 'Système')
          onClose()
        }}
        style={menuItemStyle}
        onMouseEnter={hoverItem}
        onMouseLeave={unhoverItem}
      >
        <FluentIcon name="refresh" size={15} />
        <span>Actualiser</span>
      </div>

      <div style={separatorStyle} />

      {/* Nouveau dossier */}
      <div
        onClick={() => {
          createFolder('C:/Users/eemntirey/Desktop', `Nouveau dossier (${Date.now().toString().slice(-4)})`)
          onClose()
        }}
        style={menuItemStyle}
        onMouseEnter={hoverItem}
        onMouseLeave={unhoverItem}
      >
        <FluentIcon name="folder" size={15} />
        <span>Nouveau dossier</span>
      </div>

      {/* Nouveau document texte */}
      <div
        onClick={() => {
          createFile(
            'C:/Users/eemntirey/Desktop',
            `Nouveau document (${Date.now().toString().slice(-4)}).txt`,
            ''
          )
          onClose()
        }}
        style={menuItemStyle}
        onMouseEnter={hoverItem}
        onMouseLeave={unhoverItem}
      >
        <FluentIcon name="file-text" size={15} />
        <span>Nouveau document texte</span>
      </div>

      <div style={separatorStyle} />

      {/* Ouvrir dans le Terminal */}
      <div
        onClick={() => {
          openWindow('terminal')
          onClose()
        }}
        style={menuItemStyle}
        onMouseEnter={hoverItem}
        onMouseLeave={unhoverItem}
      >
        <FluentIcon name="terminal" size={15} />
        <span>Ouvrir dans le Terminal Windows</span>
      </div>

      {/* Paramètres d'affichage - Afficher/Masquer le libellé */}
      <div
        onClick={() => {
          toggleDesktopLabels()
          onClose()
        }}
        style={menuItemStyle}
        onMouseEnter={hoverItem}
        onMouseLeave={unhoverItem}
      >
        <FluentIcon name="sliders" size={15} />
        <span>{showDesktopLabels ? 'Masquer le libellé' : 'Afficher le libellé'}</span>
      </div>

      <div style={separatorStyle} />

      {/* Taille des icônes */}
      <div style={{ padding: '4px 12px', fontSize: '11px', fontWeight: 600, color: 'var(--text-tertiary)', letterSpacing: '0.3px' }}>Taille des icônes</div>
      {(['small', 'medium', 'large'] as const).map(size => (
        <div
          key={size}
          onClick={() => {
            onIconSizeChange?.(size)
            onClose()
          }}
          style={{
            ...menuItemStyle,
            fontWeight: iconSize === size ? 600 : 400,
            color: iconSize === size ? 'var(--win-accent-cyan)' : 'var(--text-primary)',
          }}
          onMouseEnter={hoverItem}
          onMouseLeave={unhoverItem}
        >
          <FluentIcon name={iconSize === size ? 'check' : 'circle'} size={14} />
          <span>{size === 'small' ? 'Petite' : size === 'medium' ? 'Moyenne' : 'Grande'}</span>
        </div>
      ))}

      {/* Personnaliser */}
      <div
        onClick={() => {
          openWindow('settings')
          onClose()
        }}
        style={menuItemStyle}
        onMouseEnter={hoverItem}
        onMouseLeave={unhoverItem}
      >
        <FluentIcon name="palette" size={15} />
        <span>Personnaliser</span>
      </div>
    </div>
  )
}

const menuItemStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  padding: '7px 12px',
  borderRadius: '4px',
  cursor: 'pointer',
  color: 'var(--text-primary)',
  transition: 'background 0.1s',
}

const separatorStyle: React.CSSProperties = {
  height: '1px',
  backgroundColor: 'var(--window-border)',
  margin: '4px 6px',
}

const hoverItem = (e: React.MouseEvent<HTMLDivElement>) => {
  e.currentTarget.style.backgroundColor = 'var(--control-hover)'
}

const unhoverItem = (e: React.MouseEvent<HTMLDivElement>) => {
  e.currentTarget.style.backgroundColor = 'transparent'
}
