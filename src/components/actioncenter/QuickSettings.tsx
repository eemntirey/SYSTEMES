import React from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

export const QuickSettings: React.FC = () => {
  const {
    isActionCenterOpen,
    wifiEnabled,
    toggleWifi,
    bluetoothEnabled,
    toggleBluetooth,
    nightLight,
    toggleNightLight,
    batterySaver,
    toggleBatterySaver,
    brightness,
    setBrightness,
    volume,
    setVolume,
    isMuted,
    toggleMute,
    audioDeviceName,
    openWindow,
    closeAllFlyouts,
  } = useOS()

  if (!isActionCenterOpen) return null

  const quickToggles = [
    {
      id: 'wifi',
      name: 'Wi-Fi',
      desc: wifiEnabled ? 'Connecté' : 'Désactivé',
      active: wifiEnabled,
      onClick: toggleWifi,
      icon: wifiEnabled ? 'wifi' : 'wifi-off',
    },
    {
      id: 'bluetooth',
      name: 'Bluetooth',
      desc: bluetoothEnabled ? 'Activé' : 'Désactivé',
      active: bluetoothEnabled,
      onClick: toggleBluetooth,
      icon: 'sliders',
    },
    {
      id: 'nightlight',
      name: 'Éclairage nocturne',
      desc: nightLight ? 'Activé' : 'Désactivé',
      active: nightLight,
      onClick: toggleNightLight,
      icon: 'moon',
    },
    {
      id: 'batterysaver',
      name: 'Éco batterie',
      desc: batterySaver ? 'Activé' : 'Désactivé',
      active: batterySaver,
      onClick: toggleBatterySaver,
      icon: 'battery-charging',
    },
  ]

  return (
    <div
      onClick={e => e.stopPropagation()}
      style={{
        position: 'fixed',
        bottom: '56px',
        right: '8px',
        width: '350px',
        backgroundColor: '#1a1a1a',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: '12px',
        boxShadow: '0 8px 20px rgba(0, 0, 0, 0.2)',
        zIndex: 1000,
        padding: '12px',
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
        animation: 'slideRightFade 0.16s ease-out',
      }}
    >
      {/* Toggles Grid */}
      <div
        style={{
          display: 'grid',
          gridTemplateColumns: '1fr 1fr',
          gap: '10px',
        }}
      >
        {quickToggles.map(toggle => (
          <button
            key={toggle.id}
            type="button"
            aria-pressed={toggle.active}
            onClick={toggle.onClick}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '10px',
              padding: '10px 12px',
              borderRadius: '8px',
              backgroundColor: toggle.active ? 'var(--win-accent)' : 'var(--flyout-card)',
              color: toggle.active ? '#ffffff' : 'var(--text-primary)',
              cursor: 'pointer',
              border: '1px solid var(--window-border)',
              transition: 'background 0.12s',
              textAlign: 'left',
              font: 'inherit',
            }}
          >
            <FluentIcon name={toggle.icon} size={16} />
            <div style={{ overflow: 'hidden' }}>
              <div style={{ fontSize: '12px', fontWeight: 600, whiteSpace: 'nowrap' }}>
                {toggle.name}
              </div>
              <div
                style={{
                  fontSize: '10.5px',
                  color: toggle.active ? 'rgba(255,255,255,0.8)' : 'var(--text-tertiary)',
                  whiteSpace: 'nowrap',
                }}
              >
                {toggle.desc}
              </div>
            </div>
          </button>
        ))}
      </div>

      {/* Sliders Area */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
        {/* Brightness Slider */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <FluentIcon name="sun" size={17} color="var(--text-secondary)" />
          <label style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1 }}>
          <span style={{ fontSize: 11, color: 'var(--text-secondary)', minWidth: 62 }}>Luminosité</span>
          <input
            aria-label="Luminosité"
            type="range"
            min="20"
            max="100"
            value={brightness}
            onChange={e => setBrightness(Number(e.target.value))}
            style={{ flex: 1, accentColor: 'var(--win-accent)', cursor: 'pointer' }}
          />
          <span style={{ fontSize: 11, color: 'var(--text-secondary)', minWidth: 32 }}>{brightness}%</span>
          </label>
        </div>

        {/* Volume Slider */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <button
            onClick={toggleMute}
            style={{ border: 'none', background: 'transparent', cursor: 'pointer', color: 'var(--text-secondary)' }}
          >
            <FluentIcon name={isMuted || volume === 0 ? 'volume-x' : 'volume'} size={17} />
          </button>
          <label style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1 }}>
          <span style={{ fontSize: 11, color: 'var(--text-secondary)', minWidth: 62 }}>Volume</span>
          <input
            aria-label="Volume"
            type="range"
            min="0"
            max="100"
            value={isMuted ? 0 : volume}
            onChange={e => setVolume(Number(e.target.value))}
            style={{ flex: 1, accentColor: 'var(--win-accent)', cursor: 'pointer' }}
          />
          <span style={{ fontSize: 11, color: 'var(--text-secondary)', minWidth: 32 }}>{isMuted ? 0 : volume}%</span>
          </label>
        </div>

        {audioDeviceName && (
          <div
            style={{
              fontSize: 10.5,
              color: 'var(--text-tertiary)',
              marginTop: -6,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
            title={audioDeviceName}
          >
            Sortie : {audioDeviceName}
          </div>
        )}
      </div>

      {/* Footer Info: Battery & Settings link */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          paddingTop: '12px',
          borderTop: '1px solid var(--window-border)',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px' }}>
          <FluentIcon name="battery" size={16} />
          <span>État de la batterie simulé · 94 %</span>
        </div>

        <button
          onClick={() => {
            openWindow('settings')
            closeAllFlyouts()
          }}
          title="Tous les paramètres"
          style={{
            border: 'none',
            background: 'transparent',
            color: 'var(--text-secondary)',
            cursor: 'pointer',
            padding: '4px',
            borderRadius: '4px',
          }}
          onMouseEnter={e => (e.currentTarget.style.color = 'var(--text-primary)')}
          onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-secondary)')}
        >
          <FluentIcon name="settings" size={16} />
        </button>
      </div>
    </div>
  )
}
