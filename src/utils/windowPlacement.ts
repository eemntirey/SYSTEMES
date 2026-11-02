/**
 * Placement multi-écrans — coords alignées sur le bureau virtuel Windows
 * (virtualScreen.x/y peuvent être négatifs).
 */
export type Rect = { x: number; y: number; width: number; height: number }

export type MonitorInfo = {
  index: number
  device?: string
  isPrimary: boolean
  bounds: Rect
  workArea: Rect
  resolution?: string
}

export type DisplayLayout = {
  resolution: string
  width: number
  height: number
  monitorCount: number
  virtualScreen: Rect
  monitors: MonitorInfo[]
  source?: string
}

export type SnapPosition =
  | 'left-half'
  | 'right-half'
  | 'top-left'
  | 'top-right'
  | 'bottom-left'
  | 'bottom-right'

const TASKBAR_RESERVE = 56
const MENU_BAR = 0

/** Layout de secours = viewport navigateur (1 écran). */
export function fallbackLayout(): DisplayLayout {
  const w = typeof window !== 'undefined' ? window.innerWidth : 1920
  const h = typeof window !== 'undefined' ? window.innerHeight : 1080
  const workH = Math.max(200, h - TASKBAR_RESERVE - MENU_BAR)
  const bounds = { x: 0, y: 0, width: w, height: h }
  const workArea = { x: 0, y: MENU_BAR, width: w, height: workH }
  return {
    resolution: `${w} x ${h}`,
    width: w,
    height: h,
    monitorCount: 1,
    virtualScreen: { ...bounds },
    monitors: [
      {
        index: 0,
        isPrimary: true,
        bounds,
        workArea,
        resolution: `${w} x ${h}`,
      },
    ],
    source: 'fallback-viewport',
  }
}

export function normalizeLayout(raw: Partial<DisplayLayout> | null | undefined): DisplayLayout {
  if (!raw || !Array.isArray(raw.monitors) || raw.monitors.length === 0) {
    return fallbackLayout()
  }
  const monitors: MonitorInfo[] = raw.monitors.map((m, i) => {
    const b = m.bounds || { x: 0, y: 0, width: raw.width || 1920, height: raw.height || 1080 }
    const wa = m.workArea || {
      x: b.x,
      y: b.y + MENU_BAR,
      width: b.width,
      height: Math.max(200, b.height - TASKBAR_RESERVE - MENU_BAR),
    }
    const workArea = {
      x: wa.x,
      y: wa.y,
      width: wa.width,
      height: Math.max(200, wa.height - (wa.height >= b.height - 8 ? TASKBAR_RESERVE : 0)),
    }
    return {
      index: m.index ?? i,
      device: m.device,
      isPrimary: Boolean(m.isPrimary),
      bounds: { x: b.x, y: b.y, width: b.width, height: b.height },
      workArea,
      resolution: m.resolution || `${b.width} x ${b.height}`,
    }
  })
  if (!monitors.some(m => m.isPrimary)) monitors[0].isPrimary = true
  const vs = raw.virtualScreen || {
    x: Math.min(...monitors.map(m => m.bounds.x)),
    y: Math.min(...monitors.map(m => m.bounds.y)),
    width: raw.width || monitors.reduce((s, m) => Math.max(s, m.bounds.x + m.bounds.width), 0),
    height: raw.height || monitors.reduce((s, m) => Math.max(s, m.bounds.y + m.bounds.height), 0),
  }
  return {
    resolution: raw.resolution || monitors.find(m => m.isPrimary)!.resolution || '',
    width: raw.width || vs.width,
    height: raw.height || vs.height,
    monitorCount: monitors.length,
    virtualScreen: { x: vs.x, y: vs.y, width: vs.width, height: vs.height },
    monitors,
    source: raw.source || 'native',
  }
}

export function monitorAt(x: number, y: number, layout: DisplayLayout): MonitorInfo {
  const hit = layout.monitors.find(
    m =>
      x >= m.bounds.x &&
      x < m.bounds.x + m.bounds.width &&
      y >= m.bounds.y &&
      y < m.bounds.y + m.bounds.height,
  )
  return hit || layout.monitors.find(m => m.isPrimary) || layout.monitors[0]
}

export function primaryMonitor(layout: DisplayLayout): MonitorInfo {
  return layout.monitors.find(m => m.isPrimary) || layout.monitors[0]
}

export function snapRect(position: SnapPosition, monitor: MonitorInfo): Rect {
  const wa = monitor.workArea
  const halfW = Math.floor(wa.width / 2)
  const halfH = Math.floor(wa.height / 2)
  switch (position) {
    case 'left-half':
      return { x: wa.x, y: wa.y, width: halfW, height: wa.height }
    case 'right-half':
      return { x: wa.x + halfW, y: wa.y, width: wa.width - halfW, height: wa.height }
    case 'top-left':
      return { x: wa.x, y: wa.y, width: halfW, height: halfH }
    case 'top-right':
      return { x: wa.x + halfW, y: wa.y, width: wa.width - halfW, height: halfH }
    case 'bottom-left':
      return { x: wa.x, y: wa.y + halfH, width: halfW, height: wa.height - halfH }
    case 'bottom-right':
      return { x: wa.x + halfW, y: wa.y + halfH, width: wa.width - halfW, height: wa.height - halfH }
    default:
      return { x: wa.x, y: wa.y, width: halfW, height: wa.height }
  }
}

export function maximizeRect(monitor: MonitorInfo): Rect {
  const wa = monitor.workArea
  return { x: wa.x, y: wa.y, width: wa.width, height: wa.height }
}

export function clampPosition(
  x: number,
  y: number,
  winW: number,
  _winH: number,
  layout: DisplayLayout,
): { x: number; y: number } {
  const vs = layout.virtualScreen
  const minVisible = 80
  const minX = vs.x - winW + minVisible
  const maxX = vs.x + vs.width - minVisible
  const minY = vs.y
  const maxY = vs.y + vs.height - 40
  return {
    x: Math.min(maxX, Math.max(minX, x)),
    y: Math.min(maxY, Math.max(minY, y)),
  }
}

export function cascadeOpenPosition(
  layout: DisplayLayout,
  windowCount: number,
  size: { width: number; height: number },
): { x: number; y: number } {
  const m = primaryMonitor(layout)
  const offset = (windowCount % 8) * 28
  const x = m.workArea.x + 72 + offset
  const y = m.workArea.y + 48 + offset
  return {
    x: Math.min(x, m.workArea.x + Math.max(0, m.workArea.width - size.width)),
    y: Math.min(y, m.workArea.y + Math.max(0, m.workArea.height - size.height)),
  }
}

export function toCssOrigin(layout: DisplayLayout): { ox: number; oy: number } {
  if (layout.source === 'fallback-viewport') return { ox: 0, oy: 0 }
  const vs = layout.virtualScreen
  if (typeof window !== 'undefined') {
    const vw = window.innerWidth
    const vh = window.innerHeight
    if (Math.abs(vw - vs.width) < 4 && Math.abs(vh - vs.height) < 4) {
      return { ox: vs.x, oy: vs.y }
    }
  }
  return { ox: 0, oy: 0 }
}

export function toCssRect(rect: Rect, layout: DisplayLayout): Rect {
  const { ox, oy } = toCssOrigin(layout)
  return {
    x: rect.x - ox,
    y: rect.y - oy,
    width: rect.width,
    height: rect.height,
  }
}

export function fromCssPoint(cssX: number, cssY: number, layout: DisplayLayout): { x: number; y: number } {
  const { ox, oy } = toCssOrigin(layout)
  return { x: cssX + ox, y: cssY + oy }
}

export { TASKBAR_RESERVE, MENU_BAR }
