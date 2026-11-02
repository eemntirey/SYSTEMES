import React, { useState } from 'react'
import { FluentIcon } from '../common/FluentIcon'
import { NativeBridge } from '../../services/NativeBridge'
import {
  DisplayNativePanel,
  InstalledAppsNativePanel,
  StorageNativePanel,
  DefaultAppsOpenRow,
} from './SettingsNativePanels'

type Tab = 'display' | 'apps' | 'storage'

const tabs: { id: Tab; label: string; icon: string }[] = [
  { id: 'display', label: 'Écrans', icon: 'monitor' },
  { id: 'apps', label: 'Applications', icon: 'grid' },
  { id: 'storage', label: 'Stockage', icon: 'hard-drive' },
]

export const SettingsApp: React.FC = () => {
  const [tab, setTab] = useState<Tab>('display')

  return (
    <div
      style={{
        display: 'flex',
        height: '100%',
        minHeight: 0,
        background: 'var(--window-bg)',
        color: 'var(--text-primary)',
        fontFamily: 'var(--font-segoe, Segoe UI, sans-serif)',
      }}
    >
      <aside
        style={{
          width: 220,
          flexShrink: 0,
          borderRight: '1px solid var(--window-border)',
          background: 'var(--flyout-card)',
          padding: '16px 10px',
          display: 'flex',
          flexDirection: 'column',
          gap: 6,
        }}
      >
        <div style={{ fontSize: 13, fontWeight: 650, padding: '6px 10px', marginBottom: 8 }}>Paramètres</div>
        {tabs.map(t => (
          <button
            key={t.id}
            type="button"
            onClick={() => setTab(t.id)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 10,
              padding: '10px 12px',
              border: 'none',
              borderRadius: 6,
              background: tab === t.id ? 'rgba(139,92,246,0.18)' : 'transparent',
              color: 'var(--text-primary)',
              cursor: 'pointer',
              textAlign: 'left',
              fontSize: 13,
            }}
          >
            <FluentIcon name={t.icon} size={16} color="var(--win-accent)" />
            {t.label}
          </button>
        ))}
        <div style={{ flex: 1 }} />
        <button
          type="button"
          onClick={() => void NativeBridge.openWindowsSettings('about')}
          style={{
            border: '1px solid var(--window-border)',
            borderRadius: 6,
            padding: '8px 10px',
            background: 'transparent',
            color: 'var(--text-secondary)',
            cursor: 'pointer',
            fontSize: 12,
          }}
        >
          Ouvrir Paramètres Windows…
        </button>
      </aside>

      <main style={{ flex: 1, overflow: 'auto', padding: '24px 28px 40px' }}>
        <h1 style={{ fontSize: 26, fontWeight: 650, margin: '0 0 6px' }}>
          {tabs.find(t => t.id === tab)?.label}
        </h1>
        <p style={{ color: 'var(--text-secondary)', fontSize: 12, margin: '0 0 20px' }}>
          Données lues depuis Windows (moteur natif) — lecture seule.
        </p>
        {tab === 'display' && <DisplayNativePanel />}
        {tab === 'apps' && (
          <>
            <InstalledAppsNativePanel />
            <DefaultAppsOpenRow onOpen={() => void NativeBridge.openWindowsSettings('default-apps')} />
          </>
        )}
        {tab === 'storage' && <StorageNativePanel />}
      </main>
    </div>
  )
}
