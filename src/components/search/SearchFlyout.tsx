import React, { useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { AppId } from '../../types'

export const SearchFlyout: React.FC = () => {
  const { isSearchOpen, openWindow, closeAllFlyouts, files } = useOS()
  const [query, setQuery] = useState('')

  if (!isSearchOpen) return null

  const topApps: { id: AppId; name: string; icon: string }[] = [
    { id: 'edge', name: 'Microsoft Edge', icon: 'globe' },
    { id: 'explorer', name: 'Explorateur', icon: 'folder' },
    { id: 'notepad', name: 'Bloc-notes', icon: 'file-text' },
    { id: 'terminal', name: 'Terminal', icon: 'terminal' },
    { id: 'calculator', name: 'Calculatrice', icon: 'calculator' },
    { id: 'settings', name: 'Paramètres', icon: 'settings' },
  ]

  const matchedApps = topApps.filter(a =>
    a.name.toLowerCase().includes(query.toLowerCase())
  )

  const matchedFiles = files.filter(f =>
    f.name.toLowerCase().includes(query.toLowerCase())
  )

  return (
    <div
      onClick={e => e.stopPropagation()}
      style={{
        position: 'fixed',
        bottom: '104px',
        left: '50%',
        transform: 'translateX(-50%)',
        width: '640px',
        height: '560px',
        backgroundColor: 'var(--flyout-bg)',
        backdropFilter: 'var(--backdrop-blur)',
        WebkitBackdropFilter: 'var(--backdrop-blur)',
        border: '1px solid var(--flyout-border)',
        borderRadius: '12px',
        boxShadow: '0 24px 54px rgba(0,0,0,0.55)',
        zIndex: 99999,
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        animation: 'slideUpFade 0.18s cubic-bezier(0.16, 1, 0.3, 1)',
      }}
    >
      {/* Search Input Bar */}
      <div style={{ padding: '20px 24px 12px 24px' }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '12px',
            padding: '10px 16px',
            backgroundColor: 'var(--flyout-input-bg)',
            borderRadius: '24px',
            border: '1px solid var(--win-accent)',
          }}
        >
          <FluentIcon name="search" size={17} color="var(--win-accent)" />
          <input
            autoFocus
            type="text"
            placeholder="Tapez ici pour rechercher des applications, des fichiers ou le web..."
            value={query}
            onChange={e => setQuery(e.target.value)}
            style={{
              width: '100%',
              border: 'none',
              background: 'transparent',
              outline: 'none',
              fontSize: '13.5px',
              color: 'var(--text-primary)',
            }}
          />
        </div>
      </div>

      {/* Main Content */}
      <div style={{ flex: 1, padding: '0 24px 20px 24px', overflowY: 'auto' }}>
        {query.trim() ? (
          /* Live Results */
          <div>
            {matchedApps.length > 0 && (
              <div style={{ marginBottom: '20px' }}>
                <div style={{ fontSize: '13px', fontWeight: 600, marginBottom: '8px' }}>Applications</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                  {matchedApps.map(app => (
                    <div
                      key={app.id}
                      onClick={() => {
                        openWindow(app.id)
                        closeAllFlyouts()
                      }}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '12px',
                        padding: '10px 14px',
                        borderRadius: '6px',
                        backgroundColor: 'var(--flyout-card)',
                        cursor: 'pointer',
                      }}
                    >
                      <FluentIcon name={app.icon} size={24} />
                      <span style={{ fontSize: '13px', fontWeight: 500 }}>{app.name}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {matchedFiles.length > 0 && (
              <div>
                <div style={{ fontSize: '13px', fontWeight: 600, marginBottom: '8px' }}>Fichiers</div>
                <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                  {matchedFiles.map(file => (
                    <div
                      key={file.id}
                      onClick={() => {
                        openWindow('notepad', { fileId: file.id, filePath: file.path, fileContent: file.content || '' })
                        closeAllFlyouts()
                      }}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '12px',
                        padding: '10px 14px',
                        borderRadius: '6px',
                        backgroundColor: 'var(--flyout-card)',
                        cursor: 'pointer',
                      }}
                    >
                      <FluentIcon name="file-text" size={24} color="var(--win-accent)" />
                      <div>
                        <div style={{ fontSize: '13px', fontWeight: 500 }}>{file.name}</div>
                        <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>{file.path}</div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {matchedApps.length === 0 && matchedFiles.length === 0 && (
              <div
                onClick={() => {
                  openWindow('edge')
                  closeAllFlyouts()
                }}
                style={{
                  padding: '16px',
                  borderRadius: '8px',
                  backgroundColor: 'var(--flyout-card)',
                  cursor: 'pointer',
                  textAlign: 'center',
                }}
              >
                <div style={{ fontSize: '14px', color: 'var(--win-accent)', fontWeight: 500 }}>
                  Rechercher sur Bing : « {query} »
                </div>
                <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                  Ouvrir les résultats dans Microsoft Edge
                </div>
              </div>
            )}
          </div>
        ) : (
          /* Default Top Searches View */
          <div>
            <div style={{ fontSize: '13px', fontWeight: 600, marginBottom: '12px' }}>
              Applications les plus utilisées
            </div>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(6, 1fr)',
                gap: '8px',
                marginBottom: '24px',
              }}
            >
              {topApps.map(app => (
                <div
                  key={app.id}
                  onClick={() => {
                    openWindow(app.id)
                    closeAllFlyouts()
                  }}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    padding: '12px 6px',
                    borderRadius: '8px',
                    backgroundColor: 'var(--flyout-card)',
                    cursor: 'pointer',
                    transition: 'background 0.12s',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--flyout-card-hover)')}
                  onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'var(--flyout-card)')}
                >
                  <FluentIcon name={app.icon} size={28} />
                  <span
                    style={{
                      fontSize: '11px',
                      marginTop: '6px',
                      textAlign: 'center',
                      whiteSpace: 'nowrap',
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      maxWidth: '70px',
                    }}
                  >
                    {app.name}
                  </span>
                </div>
              ))}
            </div>

            <div style={{ fontSize: '13px', fontWeight: 600, marginBottom: '10px' }}>
              Recherches rapides
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
              {[
                { title: "Actualités aujourd'hui", desc: 'Découvrir les titres récents' },
                { title: 'Calculer ou convertir', desc: 'Devises, unités, pourcentages' },
                { title: 'Météo ce week-end', desc: 'Prévisions locales complètes' },
              ].map(item => (
                <div
                  key={item.title}
                  onClick={() => {
                    openWindow('edge')
                    closeAllFlyouts()
                  }}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '12px',
                    padding: '10px 14px',
                    borderRadius: '6px',
                    backgroundColor: 'var(--flyout-card)',
                    cursor: 'pointer',
                  }}
                >
                  <FluentIcon name="globe" size={16} color="var(--win-accent)" />
                  <div>
                    <div style={{ fontSize: '12.5px', fontWeight: 500 }}>{item.title}</div>
                    <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>{item.desc}</div>
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
