import React, { useState } from 'react'
import { FluentIcon } from '../common/FluentIcon'
import { useOS } from '../../context/OSContext'

export type DesktopSortMode = 'name' | 'size' | 'type' | 'modified'

interface ContextMenuProps {
  x: number
  y: number
  onClose: () => void
  iconSize?: 'small' | 'medium' | 'large'
  onIconSizeChange?: (size: 'small' | 'medium' | 'large') => void
  sortBy?: DesktopSortMode
  onSortChange?: (mode: DesktopSortMode) => void
}

type SubMenuId = 'view' | 'sort' | 'new' | null

/**
 * Menu contextuel du bureau — reproduction du menu Windows 11 (24H2) :
 * rangée d'actions en haut (Couper/Copier/Coller/Renommer/Supprimer/Partager),
 * sous-menus « Afficher › », « Trier par › » et « Nouveau › »,
 * puis Actualiser / Ouvrir dans le Terminal / Paramètres d'affichage / Personnaliser.
 */
export const ContextMenu: React.FC<ContextMenuProps> = ({
  x,
  y,
  onClose,
  iconSize = 'medium',
  onIconSizeChange,
  sortBy = 'name',
  onSortChange,
}) => {
  const { createFolder, createFile, openWindow, addNotification } = useOS()
  const [openSub, setOpenSub] = useState<SubMenuId>(null)

  // Garde le menu dans l'écran
  const menuWidth = 232
  const menuHeight = 420
  const adjustedX = Math.min(x, window.innerWidth - menuWidth - 170)
  const adjustedY = Math.min(Math.max(y, 8), window.innerHeight - menuHeight - 56)

  // Rangée d'actions : sans sélection, Windows 11 grise tout (comportement réel).
  const topActions: Array<{ icon: string; label: string }> = [
    { icon: 'cut', label: 'Couper' },
    { icon: 'copy', label: 'Copier' },
    { icon: 'clipboard', label: 'Coller' },
    { icon: 'edit', label: 'Renommer' },
    { icon: 'trash', label: 'Supprimer' },
    { icon: 'share', label: 'Partager' },
    { icon: 'square', label: 'Ouvrir dans une nouvelle fenêtre' },
  ]

  const iconSizes: Array<{ value: 'small' | 'medium' | 'large'; label: string }> = [
    { value: 'small', label: 'Petite' },
    { value: 'medium', label: 'Moyenne' },
    { value: 'large', label: 'Grande' },
  ]

  const sortModes: Array<{ value: DesktopSortMode; label: string }> = [
    { value: 'name', label: 'Nom' },
    { value: 'size', label: 'Taille' },
    { value: 'type', label: 'Type de fichier' },
    { value: 'modified', label: 'Date de modification' },
  ]

  return (
    <div
      onClick={e => e.stopPropagation()}
      onMouseDown={e => e.stopPropagation()}
      onMouseLeave={() => setOpenSub(null)}
      style={{
        position: 'fixed',
        left: `${adjustedX}px`,
        top: `${adjustedY}px`,
        width: `${menuWidth}px`,
        backgroundColor: 'var(--flyout-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: '8px',
        padding: '4px',
        boxShadow: '0 12px 32px rgba(0,0,0,0.5)',
        zIndex: 99999,
        pointerEvents: 'auto',
        fontSize: '12.5px',
        animation: 'flyoutOpen 0.14s ease-out',
      }}
    >
      {/* ===== Rangée d'actions Windows 11 ===== */}
      <div style={{ display: 'flex', gap: '2px', padding: '3px 4px 6px' }}>
        {topActions.map(a => (
          <div
            key={a.label}
            title={a.label}
            aria-label={a.label}
            style={{
              width: '30px',
              height: '30px',
              borderRadius: '4px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--text-secondary)',
              opacity: 0.38,
              cursor: 'default',
            }}
          >
            <FluentIcon name={a.icon} size={15} />
          </div>
        ))}
      </div>

      <div style={separatorStyle} />

      {/* ===== Afficher › ===== */}
      <SubMenuItem
        icon="eye"
        label="Afficher"
        open={openSub === 'view'}
        onHover={() => setOpenSub('view')}
      >
        {iconSizes.map(s => (
          <MenuItem
            key={s.value}
            icon={iconSize === s.value ? 'radio-on' : 'radio-off'}
            label={s.label}
            onClick={() => {
              onIconSizeChange?.(s.value)
              onClose()
            }}
          />
        ))}
        <div style={separatorStyle} />
        <MenuItem icon="check" label="Auto disposition" dimmed />
        <MenuItem icon="check" label="Aligner sur la grille" dimmed />
        <MenuItem
          icon="check"
          label="Afficher les icônes du bureau"
          onClick={() => {
            addNotification('Bureau', 'Les icônes du bureau restent affichées', 'Explorateur')
            onClose()
          }}
        />
      </SubMenuItem>

      {/* ===== Trier par › ===== */}
      <SubMenuItem
        icon="sort"
        label="Trier par"
        open={openSub === 'sort'}
        onHover={() => setOpenSub('sort')}
      >
        {sortModes.map(m => (
          <MenuItem
            key={m.value}
            icon={sortBy === m.value ? 'radio-on' : 'radio-off'}
            label={m.label}
            onClick={() => {
              onSortChange?.(m.value)
              onClose()
            }}
          />
        ))}
      </SubMenuItem>

      {/* ===== Actualiser ===== */}
      <MenuItem
        icon="refresh"
        label="Actualiser"
        onClick={() => {
          addNotification('Bureau', 'Bureau actualisé', 'Explorateur')
          onClose()
        }}
      />

      <div style={separatorStyle} />

      {/* ===== Nouveau › ===== */}
      <SubMenuItem icon="folder" label="Nouveau" open={openSub === 'new'} onHover={() => setOpenSub('new')}>
        <MenuItem
          icon="folder"
          label="Dossier"
          onClick={() => {
            createFolder('C:/Users/eemntirey/Desktop', `Nouveau dossier (${Date.now().toString().slice(-4)})`)
            onClose()
          }}
        />
        <MenuItem icon="plus" label="Raccourci" dimmed />
        <MenuItem
          icon="file-text"
          label="Document texte"
          onClick={() => {
            createFile(
              'C:/Users/eemntirey/Desktop',
              `Nouveau document (${Date.now().toString().slice(-4)}).txt`,
              ''
            )
            onClose()
          }}
        />
      </SubMenuItem>

      <div style={separatorStyle} />

      {/* ===== Entrées système ===== */}
      <MenuItem
        icon="terminal"
        label="Ouvrir dans le Terminal"
        onClick={() => {
          openWindow('terminal')
          onClose()
        }}
      />
      <MenuItem
        icon="monitor"
        label="Paramètres d'affichage"
        onClick={() => {
          openWindow('settings')
          onClose()
        }}
      />
      <MenuItem
        icon="palette"
        label="Personnaliser"
        onClick={() => {
          openWindow('settings')
          onClose()
        }}
      />
    </div>
  )
}

/* ---------- Sous-composants ---------- */

const SubMenuItem: React.FC<{
  icon: string
  label: string
  open: boolean
  onHover: () => void
  children: React.ReactNode
}> = ({ icon, label, open, onHover, children }) => (
  <div
    onMouseEnter={onHover}
    style={{ ...menuItemStyle, position: 'relative' }}
  >
    <FluentIcon name={icon} size={15} />
    <span style={{ flex: 1 }}>{label}</span>
    <FluentIcon name="chevron-right" size={13} color="var(--text-secondary)" />

    {open && (
      <div
        style={{
          position: 'absolute',
          left: 'calc(100% + 2px)',
          top: '-5px',
          minWidth: '200px',
          backgroundColor: 'var(--flyout-bg)',
          backdropFilter: 'var(--backdrop-blur)',
          WebkitBackdropFilter: 'var(--backdrop-blur)',
          border: '1px solid var(--flyout-border)',
          borderRadius: '8px',
          padding: '4px',
          boxShadow: '0 12px 32px rgba(0,0,0,0.5)',
          zIndex: 100000,
          animation: 'flyoutOpen 0.12s ease-out',
        }}
      >
        {children}
      </div>
    )}
  </div>
)

const MenuItem: React.FC<{
  icon?: string
  label: string
  onClick?: () => void
  dimmed?: boolean
}> = ({ icon, label, onClick, dimmed }) => (
  <div
    onClick={onClick}
    style={{
      ...menuItemStyle,
      opacity: dimmed ? 0.45 : 1,
      cursor: dimmed ? 'default' : 'pointer',
    }}
    onMouseEnter={e => {
      if (!dimmed) e.currentTarget.style.backgroundColor = 'var(--control-hover)'
    }}
    onMouseLeave={e => {
      e.currentTarget.style.backgroundColor = 'transparent'
    }}
  >
    {icon && (
      <FluentIcon
        name={icon}
        size={14}
        color={icon === 'radio-on' ? 'var(--win-accent-cyan)' : 'var(--text-secondary)'}
      />
    )}
    <span style={{ color: 'var(--text-primary)' }}>{label}</span>
  </div>
)

/* ---------- Styles partagés ---------- */

const menuItemStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: '10px',
  padding: '7px 10px',
  borderRadius: '4px',
  cursor: 'pointer',
  color: 'var(--text-primary)',
  transition: 'background 0.1s',
  whiteSpace: 'nowrap',
}

const separatorStyle: React.CSSProperties = {
  height: '1px',
  backgroundColor: 'var(--window-border)',
  margin: '4px 6px',
}
