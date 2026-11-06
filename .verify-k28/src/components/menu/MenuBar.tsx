import React, { useState, useEffect } from 'react'
import { FluentIcon } from '../common/FluentIcon'
import { useOS } from '../../context/OSContext'

export const MenuBar: React.FC = () => {
  const { windows, activeWindowId, toggleStartMenu, toggleActionCenter } = useOS()
  const [timeStr, setTimeStr] = useState('')
  const [dateStr, setDateStr] = useState('')

  useEffect(() => {
    const tick = () => {
      const now = new Date()
      setTimeStr(now.toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' }))
      setDateStr(now.toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }))
    }
    tick()
    const id = setInterval(tick, 1000)
    return () => clearInterval(id)
  }, [])
  const activeWindow = windows.find(w => w.id === activeWindowId)
  const activeAppName = activeWindow?.title || 'Windows 11'
  const activeIcon = activeWindow?.icon || 'windows-logo'

  return (
    <header role="banner" aria-label="Barre de menu" style={{
      position: 'fixed', top: 0, left: 0, width: '100%', height: '40px',
      background: 'rgba(10, 10, 18, 0.85)', backdropFilter: 'blur(20px) saturate(160%)', WebkitBackdropFilter: 'blur(20px) saturate(160%)',
      color: '#f0f0f5', fontFamily: "'Inter', '-apple-system', 'Segoe UI', 'BlinkMacSystemFont', 'Roboto', 'Helvetica Neue', sans-serif",
      fontSize: '13px', fontWeight: 500, zIndex: 99995, userSelect: 'none', display: 'flex',
      alignItems: 'center', justifyContent: 'space-between', padding: '0 20px',
      boxShadow: '0 1px 4px rgba(0,0,0,0.18), inset 0 -1px 0 rgba(255,255,255,0.06)', borderBottom: '1px solid rgba(255,255,255,0.08)',
      contain: 'layout', letterSpacing: '0.3px'
    }}>
      <nav aria-label="Navigation" style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1 }}>
        <button aria-label="Ouvrir le menu Démarrer" onClick={toggleStartMenu} title="Menu Démarrer" style={{ background: 'rgba(255,255,255,0.06)', border: '1px solid rgba(255,255,255,0.08)', borderRadius: 6, color: '#f0f0f5', cursor: 'pointer', padding: '5px 12px', fontSize: '12px', fontWeight: 500, fontFamily: 'inherit', display: 'flex', alignItems: 'center', gap: 8, transition: 'background 0.2s ease, border-color 0.2s ease', boxShadow: '0 1px 2px rgba(0,0,0,0.1)' }}
          onMouseEnter={e => { e.currentTarget.style.background = 'rgba(255,255,255,0.12)'; e.currentTarget.style.borderColor = 'rgba(255,255,255,0.15)'; }}
          onMouseLeave={e => { e.currentTarget.style.background = 'rgba(255,255,255,0.06)'; e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)'; }}>
          <FluentIcon name="windows-logo" size={14} color="#00d4ff" />
          <span>Démarrer</span>
        </button>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div style={{ width: 22, height: 22, borderRadius: 6, background: 'linear-gradient(135deg, rgba(255,255,255,0.08), rgba(255,255,255,0.03))', border: '1px solid rgba(255,255,255,0.12)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, boxShadow: '0 0 8px rgba(255,255,255,0.08)' }}>
            <FluentIcon name={activeIcon} size={13} color="#fff" />
          </div>
          <span style={{ fontWeight: 500, fontSize: '13px', color: '#f0f0f5', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: 160, opacity: 0.95 }}>{activeAppName}</span>
        </div>
      </nav>
      <aside role="status" aria-live="polite" style={{ display: 'flex', alignItems: 'center', gap: 14, flex: 1, justifyContent: 'center', fontSize: '12px', letterSpacing: '0.2px', opacity: 0.95 }}>
        <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', lineHeight: 1.2 }}>
          <time style={{ fontWeight: 500, color: '#f0f0f5' }}>{timeStr}</time>
          <span style={{ fontSize: '10px', color: 'rgba(240,240,245,0.65)' }}>{dateStr}</span>
        </div>
      </aside>
      <nav aria-label="Contrôle" style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1, justifyContent: 'flex-end' }}>
        <button aria-label="Centre de notifications" onClick={toggleActionCenter} title="Centre de notifications" style={{ background: 'transparent', border: '1px solid rgba(255,255,255,0.1)', borderRadius: 6, color: '#f0f0f5', cursor: 'pointer', padding: '5px 12px', fontSize: '12px', fontWeight: 500, fontFamily: 'inherit', display: 'flex', alignItems: 'center', gap: 6, transition: 'all 0.2s ease', boxShadow: '0 1px 3px rgba(0,0,0,0.15)' }}
          onMouseEnter={e => { e.currentTarget.style.background = 'rgba(255,255,255,0.08)'; e.currentTarget.style.borderColor = 'rgba(255,255,255,0.2)'; }}
          onMouseLeave={e => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.borderColor = 'rgba(255,255,255,0.1)'; }}>
          <FluentIcon name="bell" size={14} color="#fff" />
          <span>Centre</span>
        </button>
      </nav>
    </header>
  )
}
