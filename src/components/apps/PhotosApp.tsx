import React, { useMemo, useState } from 'react'
import { WALLPAPERS } from '../../data/constants'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

export const PhotosApp: React.FC = () => {
  const { wallpaper, setWallpaper } = useOS()
  const [activeId, setActiveId] = useState(wallpaper.id)
  const [zoom, setZoom] = useState(1)
  const [rotation, setRotation] = useState(0)
  const [slideshow, setSlideshow] = useState(false)
  const [showDetails, setShowDetails] = useState(true)
  const active = useMemo(() => WALLPAPERS.find(item => item.id === activeId) || WALLPAPERS[0], [activeId])

  React.useEffect(() => {
    if (!slideshow) return
    const timer = window.setInterval(() => {
      setActiveId(current => {
        const index = WALLPAPERS.findIndex(item => item.id === current)
        return WALLPAPERS[(index + 1) % WALLPAPERS.length].id
      })
      setZoom(1)
      setRotation(0)
    }, 3500)
    return () => window.clearInterval(timer)
  }, [slideshow])

  const changePhoto = (id: string) => {
    setActiveId(id)
    setZoom(1)
    setRotation(0)
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0, background: 'var(--window-bg)', color: 'var(--text-primary)' }}>
      <header style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '10px 14px', borderBottom: '1px solid var(--window-border)', flexWrap: 'wrap' }}>
        <strong style={{ marginRight: 'auto', fontSize: 14 }}>Collection</strong>
        <button onClick={() => setZoom(value => Math.max(.5, Math.round((value - .25) * 100) / 100))} title="Dézoomer" style={buttonStyle}><FluentIcon name="minus" size={15} /></button>
        <span style={{ minWidth: 42, textAlign: 'center', fontSize: 12 }}>{Math.round(zoom * 100)}%</span>
        <button onClick={() => setZoom(value => Math.min(3, Math.round((value + .25) * 100) / 100))} title="Zoomer" style={buttonStyle}><FluentIcon name="plus" size={15} /></button>
        <button onClick={() => setRotation(value => (value + 90) % 360)} title="Pivoter" style={buttonStyle}><FluentIcon name="rotate-cw" size={15} /></button>
        <button onClick={() => setSlideshow(value => !value)} title={slideshow ? 'Arrêter le diaporama' : 'Lancer le diaporama'} style={{ ...buttonStyle, color: slideshow ? 'var(--win-accent)' : 'var(--text-primary)' }}><FluentIcon name={slideshow ? 'pause' : 'play'} size={15} /></button>
        <button onClick={() => setShowDetails(value => !value)} title="Détails" style={buttonStyle}><FluentIcon name="info" size={15} /></button>
      </header>
      <div style={{ flex: 1, minHeight: 0, display: 'flex' }}>
        <main style={{ flex: 1, minWidth: 0, display: 'grid', placeItems: 'center', overflow: 'hidden', padding: 16, background: 'rgba(0,0,0,.18)' }}>
          <img src={active.url} alt={active.name} style={{ maxWidth: '100%', maxHeight: '100%', objectFit: 'contain', transform: `scale(${zoom}) rotate(${rotation}deg)`, transition: 'transform .2s ease', borderRadius: 8, boxShadow: '0 18px 48px rgba(0,0,0,.35)' }} />
        </main>
        {showDetails && <aside style={{ width: 190, padding: 14, borderLeft: '1px solid var(--window-border)', fontSize: 12 }}>
          <strong style={{ display: 'block', marginBottom: 8 }}>{active.name}</strong>
          <p style={{ color: 'var(--text-secondary)', lineHeight: 1.5 }}>Image de la collection intégrée au système.</p>
          <button onClick={() => setWallpaper(active)} style={{ ...buttonStyle, width: '100%', justifyContent: 'center', marginTop: 8, background: 'var(--win-accent)', color: '#fff' }}>Définir comme fond d’écran</button>
          <div style={{ color: 'var(--text-tertiary)', marginTop: 12 }}>Collection · {WALLPAPERS.length} images</div>
        </aside>}
      </div>
      <footer style={{ display: 'flex', gap: 8, overflowX: 'auto', padding: 10, borderTop: '1px solid var(--window-border)' }}>
        {WALLPAPERS.map(item => <button key={item.id} onClick={() => changePhoto(item.id)} aria-label={item.name} title={item.name} style={{ flex: '0 0 112px', height: 68, padding: 2, borderRadius: 7, overflow: 'hidden', border: item.id === activeId ? '2px solid var(--win-accent)' : '2px solid transparent', background: 'transparent', cursor: 'pointer' }}>
          <img src={item.url} alt="" style={{ width: '100%', height: '100%', objectFit: 'cover', borderRadius: 4 }} />
        </button>)}
      </footer>
    </div>
  )
}

const buttonStyle: React.CSSProperties = { display: 'inline-flex', alignItems: 'center', gap: 6, border: '1px solid var(--window-border)', borderRadius: 6, padding: '7px 9px', background: 'var(--flyout-card)', color: 'var(--text-primary)', cursor: 'pointer' }
