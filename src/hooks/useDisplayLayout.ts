import { useEffect, useState } from 'react'
import { readDisplayLayout } from '../native/p2'
import {
  DisplayLayout,
  fallbackLayout,
  normalizeLayout,
} from '../utils/windowPlacement'

let cached: DisplayLayout | null = null
const listeners = new Set<(l: DisplayLayout) => void>()

function emit(layout: DisplayLayout) {
  cached = layout
  listeners.forEach(fn => fn(layout))
}

/** Charge une fois le layout natif ; partage le cache entre composants. */
export async function loadDisplayLayout(force = false): Promise<DisplayLayout> {
  if (cached && !force) return cached
  try {
    const raw = await readDisplayLayout()
    const layout = normalizeLayout(raw as DisplayLayout)
    emit(layout)
    return layout
  } catch {
    const layout = fallbackLayout()
    emit(layout)
    return layout
  }
}

export function getCachedDisplayLayout(): DisplayLayout {
  return cached || fallbackLayout()
}

export function useDisplayLayout(): DisplayLayout {
  const [layout, setLayout] = useState<DisplayLayout>(() => cached || fallbackLayout())

  useEffect(() => {
    let cancelled = false
    const onUpdate = (l: DisplayLayout) => {
      if (!cancelled) setLayout(l)
    }
    listeners.add(onUpdate)
    void loadDisplayLayout().then(l => {
      if (!cancelled) setLayout(l)
    })
    const onResize = () => {
      if (!cached || cached.source === 'fallback-viewport') {
        emit(fallbackLayout())
      }
    }
    window.addEventListener('resize', onResize)
    return () => {
      cancelled = true
      listeners.delete(onUpdate)
      window.removeEventListener('resize', onResize)
    }
  }, [])

  return layout
}
