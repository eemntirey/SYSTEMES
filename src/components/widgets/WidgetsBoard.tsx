import React, { useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

export const WidgetsBoard: React.FC = () => {
  const { isWidgetsOpen, openWindow, closeAllFlyouts } = useOS()
  const [note, setNote] = useState('Penser à tester la nouvelle mise à jour de Windows 11.')

  if (!isWidgetsOpen) return null

  return (
    <div
      onClick={e => e.stopPropagation()}
      style={{
        position: 'fixed',
        bottom: '60px',
        left: '12px',
        width: '460px',
        maxHeight: 'calc(100vh - 80px)',
        backgroundColor: 'var(--flyout-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: '12px',
        boxShadow: '0 24px 60px rgba(0,0,0,0.6)',
        zIndex: 99999,
        padding: '20px',
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        overflowY: 'auto',
        animation: 'slideUpFade 0.18s ease-out',
      }}
    >
      {/* Header */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <FluentIcon name="widgets" size={18} color="var(--win-accent)" />
          <span style={{ fontSize: '15px', fontWeight: 600 }}>Widgets Windows 11</span>
        </div>
        <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>
          {new Date().toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'short' })}
        </div>
      </div>

      {/* Weather Widget */}
      <div
        style={{
          padding: '16px',
          borderRadius: '10px',
          background: 'linear-gradient(135deg, rgba(2, 132, 199, 0.35) 0%, rgba(14, 165, 233, 0.15) 100%)',
          border: '1px solid rgba(2, 132, 199, 0.3)',
          display: 'flex',
          flexDirection: 'column',
          gap: '12px',
        }}
      >
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <div>
            <div style={{ fontSize: '13px', fontWeight: 600 }}>Paris, France</div>
            <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Ensoleillé avec passages nuageux</div>
          </div>
          <div style={{ fontSize: '28px', fontWeight: 700 }}>19°C</div>
        </div>

        {/* Hourly Forecast */}
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '11px', textAlign: 'center' }}>
          {[
            { time: '12h', temp: '19°', icon: 'sun' },
            { time: '14h', temp: '21°', icon: 'sun' },
            { time: '16h', temp: '20°', icon: 'sun' },
            { time: '18h', temp: '18°', icon: 'cloud' },
            { time: '20h', temp: '16°', icon: 'moon' },
          ].map(h => (
            <div key={h.time} style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: '4px' }}>
              <span>{h.time}</span>
              <FluentIcon name={h.icon} size={15} color="#f59e0b" />
              <span style={{ fontWeight: 600 }}>{h.temp}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Stocks & Markets */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: '10px',
        }}
      >
        {[
          { sym: 'MSFT', name: 'Microsoft', val: '438.20 $', change: '+1.85%' },
          { sym: 'NVDA', name: 'NVIDIA', val: '141.50 $', change: '+3.12%' },
          { sym: 'AAPL', name: 'Apple', val: '232.10 $', change: '+0.64%' },
          { sym: 'BTC', name: 'Bitcoin', val: '72 450 $', change: '+4.20%' },
        ].map(stk => (
          <div
            key={stk.sym}
            style={{
              padding: '12px',
              borderRadius: '8px',
              backgroundColor: 'var(--flyout-card)',
              border: '1px solid var(--window-border)',
            }}
          >
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '12px', fontWeight: 600 }}>
              <span>{stk.sym}</span>
              <span style={{ color: '#10b981' }}>{stk.change}</span>
            </div>
            <div style={{ fontSize: '14px', fontWeight: 700, marginTop: '4px' }}>{stk.val}</div>
            <div style={{ fontSize: '10px', color: 'var(--text-tertiary)' }}>{stk.name}</div>
          </div>
        ))}
      </div>

      {/* Sticky Quick Note */}
      <div
        style={{
          padding: '14px',
          borderRadius: '8px',
          backgroundColor: 'rgba(234, 179, 8, 0.15)',
          border: '1px solid rgba(234, 179, 8, 0.4)',
        }}
      >
        <div style={{ fontSize: '12px', fontWeight: 600, color: '#facc15', marginBottom: '6px' }}>
          Mémo rapide
        </div>
        <textarea
          value={note}
          onChange={e => setNote(e.target.value)}
          rows={3}
          style={{
            width: '100%',
            background: 'transparent',
            border: 'none',
            outline: 'none',
            color: 'var(--text-primary)',
            fontSize: '12px',
            resize: 'none',
          }}
        />
      </div>

      {/* News Feed */}
      <div>
        <div style={{ fontSize: '13px', fontWeight: 600, marginBottom: '10px' }}>Actualités à la une</div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
          {[
            {
              title: 'Windows 11 24H2 : Les nouvelles avancées de sécurité et d’IA',
              source: 'Microsoft News • 1h',
            },
            {
              title: 'Exploration spatiale : De nouvelles images spectaculaires du télescope James Webb',
              source: 'Sciences & Avenir • 3h',
            },
            {
              title: 'Tech : Les processeurs de nouvelle génération battent des records d’efficacité',
              source: 'Clubic • 4h',
            },
          ].map((news, i) => (
            <div
              key={i}
              onClick={() => {
                openWindow('photos')
                closeAllFlyouts()
              }}
              style={{
                padding: '10px 12px',
                borderRadius: '8px',
                backgroundColor: 'var(--flyout-card)',
                border: '1px solid var(--window-border)',
                cursor: 'pointer',
                transition: 'background 0.12s',
              }}
              onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--flyout-card-hover)')}
              onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'var(--flyout-card)')}
            >
              <div style={{ fontSize: '12px', fontWeight: 500, lineHeight: '1.3' }}>{news.title}</div>
              <div style={{ fontSize: '10.5px', color: 'var(--text-tertiary)', marginTop: '4px' }}>
                {news.source}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
