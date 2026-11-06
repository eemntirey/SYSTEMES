import React from 'react'
import { FluentIcon } from '../common/FluentIcon'

export interface DesktopIconProps {
  id: string
  name: string
  icon: string
  isSelected?: boolean
  showLabel?: boolean
  onClick?: (e: React.MouseEvent) => void
  onDoubleClick?: (e: React.MouseEvent) => void
  iconSize?: 'small' | 'medium' | 'large'
}

export const DesktopIcon = React.forwardRef<HTMLDivElement, DesktopIconProps>(({
  name,
  icon,
  isSelected = false,
  showLabel = true,
  onClick,
  onDoubleClick,
  iconSize = 'medium',
}, ref) => {
  return (
    <div
      ref={ref}
      onClick={onClick}
      onDoubleClick={onDoubleClick}
      style={{
        width: '80px',
        height: '92px',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'flex-start',
        padding: '6px 4px 4px',
        borderRadius: '8px',
        cursor: 'pointer',
        backgroundColor: isSelected ? 'rgba(255, 255, 255, 0.08)' : 'transparent',
        border: isSelected ? '1px solid rgba(255, 255, 255, 0.12)' : '1px solid transparent',
        transition: 'background 0.15s ease, border-color 0.15s ease',
        userSelect: 'none',
        WebkitUserSelect: 'none',
      }}
      onMouseEnter={e => {
        if (!isSelected) {
          e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.05)'
          e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.08)'
        }
      }}
      onMouseLeave={e => {
        if (!isSelected) {
          e.currentTarget.style.backgroundColor = 'transparent'
          e.currentTarget.style.borderColor = 'transparent'
        }
      }}
    >
      <div
        style={{
          width: '48px',
          height: '48px',
          borderRadius: '12px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          transition: 'transform 0.15s ease',
        }}
      >
        <FluentIcon name={icon} size={iconSize === 'small' ? 28 : iconSize === 'large' ? 48 : 36} color={isSelected ? '#f0f0f5' : '#e0e0e8'} />
      </div>
      {showLabel && (
        <span
          style={{
            fontSize: '10.5px',
            fontWeight: 500,
            color: isSelected ? '#ffffff' : '#c8c8d8',
            textAlign: 'center',
            marginTop: '5px',
            padding: '1px 3px',
            borderRadius: '3px',
            textShadow: '0 1px 2px rgba(0, 0, 0, 0.7)',
            display: '-webkit-box',
            WebkitLineClamp: 2,
            WebkitBoxOrient: 'vertical',
            overflow: 'hidden',
            lineHeight: '1.2',
            maxWidth: '76px',
          }}
        >
          {name}
        </span>
      )}
    </div>
  )
})

