import React, { useCallback, useEffect, useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { NativeBridge } from '../../services/NativeBridge'

export const QuickSettings: React.FC = () => {
  const {
    isActionCenterOpen,
    wifiEnabled,
    toggleWifi: toggleWifiLocal,
    bluetoothEnabled,
    toggleBluetooth: toggleBluetoothLocal,
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
    addNotification,
  } = useOS()

  const [wifiOn, setWifiOn] = useState(wifiEnabled)
  const [btOn, setBtOn] = useState(bluetoothEnabled)

  useEffect(() => { setWifiOn(wifiEnabled) }, [wifiEnabled])
  useEffect(() => { setBtOn(bluetoothEnabled) }, [bluetoothEnabled])

  useEffect(() => {
    if (!isActionCenterOpen) return
    let cancelled = false
    ;(async () => {
      try {
        const wifi = await NativeBridge.readWifiStatus()
        if (!cancelled && wifi && !wifi.error) {
          setWifiOn(!!wifi.connected || (wifi.state != null && wifi.state !== ''))
        }
      } catch { /* hors host */ }
      try {
        const bt = await NativeBridge.readBluetoothStatus()
        if (!cancelled && bt && !bt.error) {
          setBtOn(!!bt.powered)
        }
      } catch { /* hors host */ }
    })()
    return () => { cancelled = true }
  }, [isActionCenterOpen])

  const handleWifi = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readWifiStatus()
        if (current?.connected) {
          await NativeBridge.disconnectWifi()
          setWifiOn(false)
          addNotification(
            'Wi-Fi',
            current.ssid ? `D\u00e9connect\u00e9 de \u00ab ${current.ssid} \u00bb.` : 'R\u00e9seau Wi-Fi d\u00e9connect\u00e9.',
            'R\u00e9seau',
          )
        } else {
          setWifiOn(true)
          try {
            await NativeBridge.openWindowsSettings('network-status')
          } catch { /* ignore */ }
          addNotification(
            'Wi-Fi',
            'Ouvrez les param\u00e8tres Windows pour vous connecter \u00e0 un r\u00e9seau.',
            'R\u00e9seau',
          )
        }
      } catch {
        toggleWifiLocal()
        setWifiOn(prev => !prev)
        addNotification('Wi-Fi', 'Moteur natif indisponible \u2014 \u00e9tat local uniquement.', 'R\u00e9seau')
      }
    })()
  }, [addNotification, toggleWifiLocal])

  const handleBluetooth = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readBluetoothStatus()
        if (current?.error) throw new Error(current.error)
        const wasPowered = !!current?.powered
        setBtOn(!wasPowered)
        try {
          await NativeBridge.openWindowsSettings('bluetooth')
        } catch { /* ignore */ }
        addNotification(
          'Bluetooth',
          wasPowered
            ? 'Param\u00e8tres Bluetooth ouverts \u2014 d\u00e9sactivez l\u2019adaptateur dans Windows si besoin.'
            : 'Param\u00e8tres Bluetooth ouverts \u2014 activez l\u2019adaptateur dans Windows.',
          'Bluetooth',
        )
        setTimeout(async () => {
          try {
            const bt = await NativeBridge.readBluetoothStatus()
            if (bt && !bt.error) setBtOn(!!bt.powered)
          } catch { /* ignore */ }
        }, 2500)
      } catch {
        toggleBluetoothLocal()
        setBtOn(prev => !prev)
        addNotification('Bluetooth', 'Moteur natif indisponible \u2014 \u00e9tat local uniquement.', 'Bluetooth')
      }
    })()
  }, [addNotification, toggleBluetoothLocal])

  if (!isActionCenterOpen) return null

  const quickToggles = [
    {
      id: 'wifi',
      name: 'Wi-Fi',
      desc: wifiOn ? 'Connect\u00e9' : 'D\u00e9sactiv\u00e9',
      active: wifiOn,
      onClick: handleWifi,
      icon: wifiOn ? 'wifi' : 'wifi-off',
    },
    {
      id: 'bluetooth',
      name: 'Bluetooth',
      desc: btOn ? 'Activ\u00e9' : 'D\u00e9sactiv\u00e9',
      active: btOn,
      onClick: handleBluetooth,
      icon: 'sliders',
    },
    {
      id: 'nightlight',
      name: '\u00c9clairage nocturne',
      desc: nightLight ? 'Activ\u00e9' : 'D\u00e9sactiv\u00e9',
      active: nightLight,
      onClick: toggleNightLight,
      icon: 'moon',
    },
    {
      id: 'batterysaver',
      name: '\u00c9co batterie',
      desc: batterySaver ? 'Activ\u00e9' : 'D\u00e9sactiv\u00e9',
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
              padding: '12px',
              borderRadius: '8px',
              border: '1px solid var(--window-border)',
              background: toggle.active ? 'var(--win-accent)' : 'var(--surface-secondary)',
              color: toggle.active ? '#fff' : 'var(--text-primary)',
              cursor: 'pointer',
              textAlign: 'left',
            }}
          >
            <FluentIcon name={toggle.icon as any} size={18} />
            <div style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0 }}>
              <span style={{ fontSize: 12, fontWeight: 600 }}>{toggle.name}</span>
              <span style={{ fontSize: 10, opacity: 0.85, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {toggle.desc}
              </span>
            </div>
          </button>
        ))}
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <FluentIcon name="sun" size={17} />
          <label style={{ display: 'flex', alignItems: 'center', gap: 12, flex: 1 }}>
            <span style={{ fontSize: 11, color: 'var(--text-secondary)', minWidth: 62 }}>Luminosit\u00e9</span>
            <input
              aria-label="Luminosit\u00e9"
              type="range"
              min="0"
              max="100"
              value={brightness}
              onChange={e => setBrightness(Number(e.target.value))}
              style={{ flex: 1, accentColor: 'var(--win-accent)', cursor: 'pointer' }}
            />
            <span style={{ fontSize: 11, color: 'var(--text-secondary)', minWidth: 32 }}>{brightness}%</span>
          </label>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <button
            type="button"
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
          <span>\u00c9tat de la batterie simul\u00e9 \u00b7 94 %</span>
        </div>

        <button
          onClick={() => {
            openWindow('settings')
            closeAllFlyouts()
          }}
          title="Tous les param\u00e8tres"
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
