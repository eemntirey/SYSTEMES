import React, { useCallback, useEffect, useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { NativeBridge } from '../../services/NativeBridge'

export const QuickSettings: React.FC = () => {
  const {
    isActionCenterOpen,
    volume,
    setVolume,
    isMuted,
    toggleMute,
    audioDeviceName,
    openWindow,
    closeAllFlyouts,
    addNotification,
  } = useOS()

  const [wifiOn, setWifiOn] = useState<boolean | null>(null)
  const [btOn, setBtOn] = useState<boolean | null>(null)

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
          await NativeBridge.openWindowsSettings('network-status')
          addNotification(
            'Wi-Fi',
            'Ouvrez les param\u00e8tres Windows pour vous connecter \u00e0 un r\u00e9seau.',
            'R\u00e9seau',
          )
        }
      } catch (error) {
        addNotification('Wi-Fi \u2014 op\u00e9ration refus\u00e9e',
          error instanceof Error ? error.message : String(error), 'R\u00e9seau')
      }
    })()
  }, [addNotification])

  const handleBluetooth = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readBluetoothStatus()
        if (current?.error) throw new Error(current.error)
        const wasPowered = !!current?.powered
        await NativeBridge.openWindowsSettings('bluetooth')
        addNotification(
          'Bluetooth',
          wasPowered
            ? 'Param\u00e8tres Bluetooth ouverts \u2014 d\u00e9sactivez l\u2019adaptateur dans Windows si besoin.'
            : 'Param\u00e8tres Bluetooth ouverts \u2014 activez l\u2019adaptateur dans Windows.',
          'Bluetooth',
        )
      } catch (error) {
        addNotification('Bluetooth \u2014 op\u00e9ration refus\u00e9e',
          error instanceof Error ? error.message : String(error), 'Bluetooth')
      }
    })()
  }, [addNotification])

  if (!isActionCenterOpen) return null

  const quickToggles = [
    {
      id: 'wifi',
      name: 'Wi-Fi',
      desc: wifiOn === null ? '\u00c9tat inconnu' : wifiOn ? 'Connect\u00e9' : 'D\u00e9sactiv\u00e9',
      active: wifiOn === true,
      onClick: handleWifi,
      icon: wifiOn ? 'wifi' : 'wifi-off',
    },
    {
      id: 'bluetooth',
      name: 'Bluetooth',
      desc: btOn === null ? '\u00c9tat inconnu' : btOn ? 'Activ\u00e9' : 'D\u00e9sactiv\u00e9',
      active: btOn === true,
      onClick: handleBluetooth,
      icon: 'sliders',
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
          <span>Batterie : état non disponible</span>
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
