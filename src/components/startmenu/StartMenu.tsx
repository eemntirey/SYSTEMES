import React, { useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { AppTile } from './AppTile'
import { APP_DEFINITIONS } from '../../data/constants'
import { AppId } from '../../types'

interface AppGridItem {
  id: AppId
  name: string
  icon: string
}

export const StartMenu: React.FC = () => {
  const {
    isStartMenuOpen,
    closeAllFlyouts,
    openWindow,
    lockOS,
    sleepOS,
    restartOS,
    shutdownOS,
    files,
  } = useOS()

  const [searchFilter, setSearchFilter] = useState('')
  const [showPowerMenu, setShowPowerMenu] = useState(false)
  const [viewAllApps, setViewAllApps] = useState(false)

  if (!isStartMenuOpen) return null

  // Le catalogue central est la source unique : pas de raccourcis fictifs ou dupliqués.
  const allUniqueApps: AppGridItem[] = Object.values(APP_DEFINITIONS)
    .map(app => ({ id: app.id, name: app.name, icon: app.icon }))
    .sort((a, b) => a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' }))
  const pinnedApps = Object.values(APP_DEFINITIONS)
    .filter(app => app.pinned)
    .map(app => ({ id: app.id, name: app.name, icon: app.icon }))

  const normalizedQuery = searchFilter.trim().toLocaleLowerCase('fr')
  const sourceApps = viewAllApps ? allUniqueApps : pinnedApps
  const filteredApps = normalizedQuery
    ? sourceApps.filter(app => app.name.toLocaleLowerCase('fr').includes(normalizedQuery))
    : sourceApps

  // Les recommandations reflètent les fichiers réellement modifiés récemment,
  // pas l'ordre arbitraire du tableau en mémoire.
  const getModifiedTime = (value: string) => {
    const match = value.match(/^(\d{2})\/(\d{2})\/(\d{4})(?:\s+(\d{2}):(\d{2}))?$/)
    if (!match) return 0
    return new Date(Number(match[3]), Number(match[2]) - 1, Number(match[1]), Number(match[4] || 0), Number(match[5] || 0)).getTime()
  }
  const recommendedFiles = [...files]
    .sort((a, b) => getModifiedTime(b.modified) - getModifiedTime(a.modified))
    .slice(0, 4)

  const handleAppClick = (appId: AppId) => {
    openWindow(appId)
    closeAllFlyouts()
  }

  return (
    <div
      onClick={e => e.stopPropagation()}
      style={{
        position: 'fixed',
        bottom: '104px',
        left: '50%',
        transform: 'translateX(-50%)',
        width: '600px',
        height: '630px',
        backgroundColor: 'var(--flyout-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: 'var(--radius-xl)',
        boxShadow: '0 24px 60px rgba(0,0,0,0.7), 0 0 30px rgba(139, 92, 246, 0.25)',
        zIndex: 99999,
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        animation: 'slideUpFade 0.2s cubic-bezier(0.16, 1, 0.3, 1)',
      }}
    >
      {/* Top Search Input */}
      <div style={{ padding: '24px 32px 14px 32px' }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '12px',
            padding: '10px 18px',
            backgroundColor: 'var(--flyout-input-bg)',
            borderRadius: 'var(--radius-xl)',
            border: '1px solid var(--flyout-border)',
            boxShadow: 'inset 0 0 12px rgba(139, 92, 246, 0.1)',
          }}
        >
          <FluentIcon name="search" size={17} color="var(--win-accent-cyan)" />
          <input
            autoFocus
            type="text"
            placeholder="Rechercher des applications, des paramètres et des fichiers..."
            value={searchFilter}
            onChange={e => setSearchFilter(e.target.value)}
            style={{
              width: '100%',
              border: 'none',
              background: 'transparent',
              outline: 'none',
              fontSize: '13px',
              color: 'var(--text-primary)',
            }}
          />
        </div>
      </div>

      {/* Main Body */}
      <div style={{ flex: 1, padding: '0 32px', overflowY: 'auto' }}>
        {/* Pinned Section Header */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: '16px',
          }}
        >
          <span style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)' }}>
            Applications épinglées
          </span>
          <button
            onClick={() => setViewAllApps(v => !v)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '4px',
              padding: '5px 12px',
              borderRadius: 'var(--radius-sm)',
              border: '1px solid var(--flyout-border)',
              backgroundColor: 'var(--flyout-card)',
              color: 'var(--text-primary)',
              fontSize: '11.5px',
              cursor: 'pointer',
            }}
          >
            <span>{viewAllApps ? 'Toutes les applications' : 'Toutes les applications >'}</span>
          </button>
        </div>

        {/* Pinned Apps Grid */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(6, 1fr)',
            gap: '12px 8px',
            marginBottom: '28px',
          }}
        >
          {(filteredApps as AppGridItem[]).map(app => (
            <AppTile
              key={app.name}
              name={app.name}
              icon={app.icon}
              onClick={() => handleAppClick(app.id)}
            />
          ))}
        </div>

        {/* Recommended Section Header */}
        <div
          style={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'center',
            marginBottom: '12px',
          }}
        >
          <span style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)' }}>
            Modifiés récemment
          </span>
          <button
            onClick={() => { openWindow('explorer'); closeAllFlyouts() }}
            style={{ fontSize: '11.5px', color: 'var(--win-accent-cyan)', cursor: 'pointer', border: 'none', background: 'transparent' }}
          >
            Ouvrir l’Explorateur &gt;
          </button>
        </div>

        {/* Recommended Items Grid */}
        <div
          style={{
            display: 'grid',
            gridTemplateColumns: 'repeat(2, 1fr)',
            gap: '10px',
            marginBottom: '16px',
          }}
        >
          {recommendedFiles.map(rf => (
            <div
              key={rf.id}
              onClick={() => {
                openWindow('notepad', { fileId: rf.id, filePath: rf.path, fileContent: rf.content || '' })
                closeAllFlyouts()
              }}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '12px',
                padding: '10px 14px',
                borderRadius: 'var(--radius-md)',
                cursor: 'pointer',
                backgroundColor: 'var(--flyout-card)',
                border: '1px solid var(--flyout-border)',
                transition: 'all 0.16s ease',
              }}
              onMouseEnter={e => {
                e.currentTarget.style.backgroundColor = 'var(--flyout-card-hover)'
                e.currentTarget.style.borderColor = 'var(--win-accent-cyan)'
                e.currentTarget.style.boxShadow = '0 0 12px rgba(0, 212, 255, 0.25)'
              }}
              onMouseLeave={e => {
                e.currentTarget.style.backgroundColor = 'var(--flyout-card)'
                e.currentTarget.style.borderColor = 'var(--flyout-border)'
                e.currentTarget.style.boxShadow = 'none'
              }}
            >
              <FluentIcon name="file-text" size={24} color="var(--win-accent-cyan)" />
              <div style={{ overflow: 'hidden' }}>
                <div
                  style={{
                    fontSize: '12.5px',
                    fontWeight: 500,
                    whiteSpace: 'nowrap',
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                  }}
                >
                  {rf.name}
                </div>
                <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>Modifié récemment</div>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* User & Power Footer */}
      <div
        style={{
          height: '66px',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 32px',
          backgroundColor: 'rgba(10, 10, 18, 0.95)',
          borderTop: '1px solid var(--flyout-border)',
          position: 'relative',
        }}
      >
        {/* User Card */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '10px',
            padding: '6px 12px',
            borderRadius: 'var(--radius-sm)',
            cursor: 'pointer',
          }}
          onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
          onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
        >
          <div
            style={{
              width: '34px',
              height: '34px',
              borderRadius: '50%',
              backgroundColor: 'var(--win-accent)',
              boxShadow: '0 0 12px var(--win-accent)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: '#ffffff',
              fontWeight: 600,
              fontSize: '13px',
            }}
          >
            U
          </div>
          <span style={{ fontSize: '13px', fontWeight: 600 }}>Utilisateur Windows</span>
        </div>

        {/* Power Button */}
        <div style={{ position: 'relative' }}>
          <button
            onClick={() => setShowPowerMenu(v => !v)}
            title="Alimentation"
            style={{
              width: '40px',
              height: '40px',
              borderRadius: 'var(--radius-sm)',
              border: showPowerMenu ? '1px solid var(--win-accent-cyan)' : 'none',
              background: showPowerMenu ? 'var(--control-hover)' : 'transparent',
              color: 'var(--text-primary)',
              cursor: 'pointer',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
            onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
            onMouseLeave={e => {
              if (!showPowerMenu) e.currentTarget.style.backgroundColor = 'transparent'
            }}
          >
            <FluentIcon name="power" size={18} color="var(--win-accent-cyan)" />
          </button>

          {/* Power Options Dropdown */}
          {showPowerMenu && (
            <div
              style={{
                position: 'absolute',
                bottom: '52px',
                right: '0',
                width: '190px',
                backgroundColor: 'var(--flyout-bg)',
                backdropFilter: 'var(--backdrop-blur)',
                border: '1px solid var(--flyout-border)',
                borderRadius: 'var(--radius-md)',
                boxShadow: '0 12px 36px rgba(0,0,0,0.65), 0 0 20px rgba(139, 92, 246, 0.3)',
                padding: '8px',
                display: 'flex',
                flexDirection: 'column',
                gap: '3px',
                zIndex: 100000,
              }}
            >
              {[
                { label: 'Verrouiller', action: lockOS, icon: 'lock' },
                { label: 'Mettre en veille', action: sleepOS, icon: 'moon' },
                { label: 'Redémarrer', action: restartOS, icon: 'restart' },
                { label: 'Arrêter', action: shutdownOS, icon: 'power' },
              ].map(opt => (
                <div
                  key={opt.label}
                  onClick={() => {
                    opt.action()
                    setShowPowerMenu(false)
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '10px',
                    padding: '8px 12px',
                    borderRadius: 'var(--radius-sm)',
                    fontSize: '12.5px',
                    cursor: 'pointer',
                    color: 'var(--text-primary)',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
                  onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
                >
                  <FluentIcon name={opt.icon} size={15} color="var(--win-accent-cyan)" />
                  <span>{opt.label}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
