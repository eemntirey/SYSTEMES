import React from 'react'
import { FluentIcon } from '../common/FluentIcon'

export interface AppTileProps {
  name: string
  icon: string
  onClick: () => void
}

export const AppTile: React.FC<AppTileProps> = ({ name, icon, onClick }) => {
  return (
    <div
      onClick={onClick}
      style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        padding: '12px 6px',
        borderRadius: 'var(--radius-md)',
        cursor: 'pointer',
        backgroundColor: 'var(--flyout-card)',
        border: '1px solid rgba(139, 92, 246, 0.12)',
        transition: 'all 0.16s ease',
      }}
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = 'var(--flyout-card-hover)'
        e.currentTarget.style.borderColor = 'var(--win-accent-cyan)'
        e.currentTarget.style.boxShadow = '0 0 16px rgba(0, 212, 255, 0.35)'
        e.currentTarget.style.transform = 'translateY(-3px)'
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'var(--flyout-card)'
        e.currentTarget.style.borderColor = 'rgba(139, 92, 246, 0.12)'
        e.currentTarget.style.boxShadow = 'none'
        e.currentTarget.style.transform = 'translateY(0)'
      }}
    >
      <FluentIcon name={icon} size={32} color="var(--win-accent-cyan)" />
      <span
        style={{
          fontSize: '11.5px',
          fontWeight: 500,
          color: 'var(--text-primary)',
          marginTop: '6px',
          textAlign: 'center',
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          maxWidth: '78px',
        }}
      >
        {name}
      </span>
    </div>
  )
}
