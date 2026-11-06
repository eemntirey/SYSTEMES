import React from 'react'
import { FluentIcon } from '../common/FluentIcon'

export interface TaskbarIconProps {
  id: string
  icon: string
  title: string
  isActive?: boolean
  isOpen?: boolean
  onClick: () => void
  onContextMenu?: (e: React.MouseEvent) => void
}

/**
 * Windows 11 taskbar app icon.
 * - Rounded-square hover/active background (Fluent style).
 * - Running indicator: small pill under the icon.
 *   Active window → accent pill; open-but-inactive → muted pill; closed → none.
 */
export const TaskbarIcon: React.FC<TaskbarIconProps> = ({
  icon,
  title,
  isActive = false,
  isOpen = false,
  onClick,
  onContextMenu,
}) => {
  return (
    <button
      onClick={onClick}
      onContextMenu={onContextMenu}
      title={title}
      aria-label={title}
      className={`taskbar-app${isActive ? ' active' : ''}`}
    >
      <FluentIcon
        name={icon}
        size={22}
        color={isActive ? 'var(--win-accent)' : 'var(--text-primary)'}
      />

      {isOpen && (
        <span
          className={`taskbar-indicator ${isActive ? 'active' : 'inactive'}`}
        />
      )}
    </button>
  )
}
