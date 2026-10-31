import React, { ReactNode } from 'react'
import { WindowFrame } from './WindowFrame'
import { WindowState } from '../../types'

export interface WindowProps {
  window: WindowState
  children: ReactNode
}

/**
 * Reusable Window component integrating apps within WindowFrame
 */
export const Window: React.FC<WindowProps> = ({ window: win, children }) => {
  return <WindowFrame window={win}>{children}</WindowFrame>
}

export default Window
