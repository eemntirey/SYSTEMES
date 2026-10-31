import React from 'react'
import { FluentIcon } from '../common/FluentIcon'

export interface TaskbarIconProps {
  id: string
  icon: string
  title: string
  isActive?: boolean
  isOpen?: boolean
  scale?: number
  onClick: () => void
  onMouseEnter?: () => void
  onMouseLeave?: () => void
}

export const TaskbarIcon: React.FC<TaskbarIconProps> = ({
  icon,
  title,
  isActive = false,
  isOpen = false,
  scale = 1,
  onClick,
  onMouseEnter,
  onMouseLeave,
}) => {
  return (
    <button
      onClick={onClick}
      title={title}
      onMouseEnter={onMouseEnter}
      onMouseLeave={onMouseLeave}
      style={{
        position: 'relative',
        width: 52,
        height: 52,
        borderRadius: '10px',
        border: isActive
          ? '1.5px solid rgba(139, 92, 246, 0.5)'
          : '1.5px solid transparent',
        backgroundColor: isActive
          ? 'rgba(139, 92, 246, 0.15)'
          : isOpen
          ? 'rgba(255, 255, 255, 0.05)'
          : 'transparent',
        boxShadow: isActive
          ? 'inset 0 0 12px rgba(139, 92, 246, 0.2)'
          : 'none',
        cursor: 'pointer',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        transition: 'all 0.2s cubic-bezier(0.34, 1.56, 0.64, 1)',
        transform: `scale(${scale})`,
        zIndex: scale > 1 ? 10 : 1,
      }}
    >
      <FluentIcon
        name={icon}
        size={scale > 1 ? 28 : 22}
        color={isActive ? '#00d4ff' : (isOpen ? '#f0f0f5' : '#c0c0d0')}
      />

      {/* Point indicator (macOS open dot) */}
      {isOpen && (
        <div
          style={{
            position: 'absolute',
            bottom: 6,
            left: '50%',
            transform: 'translateX(-50%)',
            width: isActive ? 14 : 8,
            height: 3,
            borderRadius: '2px',
            backgroundColor: isActive ? '#00d4ff' : 'rgba(255,255,255,0.5)',
            boxShadow: 'none',
            transition: 'all 0.3s ease',
          }}
        />
      )}
    </button>
  )
}
