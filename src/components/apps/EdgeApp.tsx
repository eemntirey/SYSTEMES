import React, { useState } from 'react'
import { FluentIcon } from '../common/FluentIcon'

interface Tab {
  id: string
  title: string
  url: string
}

export const EdgeApp: React.FC = () => {
  const [tabs, setTabs] = useState<Tab[]>([
    { id: 'tab-1', title: 'Nouvel onglet', url: 'edge://newtab' },
  ])
  const [activeTabId, setActiveTabId] = useState('tab-1')
  const [urlInput, setUrlInput] = useState('https://www.bing.com')
  const [searchQuery, setSearchQuery] = useState('')
  const [searchResults, setSearchResults] = useState<string | null>(null)
  const [history, setHistory] = useState<string[]>([])
  const [historyIndex, setHistoryIndex] = useState(-1)

  const handleNewTab = () => {
    const newId = `tab-${Date.now()}`
    setTabs(prev => [...prev, { id: newId, title: 'Nouvel onglet', url: 'edge://newtab' }])
    setActiveTabId(newId)
    setUrlInput('https://www.bing.com')
    setSearchResults(null)
  }

  const handleCloseTab = (e: React.MouseEvent, id: string) => {
    e.stopPropagation()
    if (tabs.length === 1) return
    const remaining = tabs.filter(t => t.id !== id)
    setTabs(remaining)
    if (activeTabId === id) {
      const newActiveId = remaining[remaining.length - 1].id
      setActiveTabId(newActiveId)
      const newActiveTab = remaining.find(t => t.id === newActiveId)
      if (newActiveTab) setUrlInput(newActiveTab.url)
    }
  }

  const handleNavigate = (targetUrl: string) => {
    let clean = targetUrl.trim()
    if (!clean.startsWith('http://') && !clean.startsWith('https://') && !clean.startsWith('edge://')) {
      clean = `https://www.bing.com/search?q=${encodeURIComponent(clean)}`
      setSearchResults(targetUrl)
    } else {
      setSearchResults(null)
    }
    setUrlInput(clean)
    setTabs(prev =>
      prev.map(t => (t.id === activeTabId ? { ...t, url: clean, title: targetUrl } : t))
    )
    setHistory(prev => {
      const next = prev.slice(0, historyIndex + 1)
      next.push(clean)
      return next
    })
    setHistoryIndex(prev => prev + 1)
  }

  const goBack = () => {
    if (historyIndex > 0) {
      const newIdx = historyIndex - 1
      setHistoryIndex(newIdx)
      const url = history[newIdx]
      handleNavigate(url)
    }
  }

  const goForward = () => {
    if (historyIndex < history.length - 1) {
      const newIdx = historyIndex + 1
      setHistoryIndex(newIdx)
      const url = history[newIdx]
      handleNavigate(url)
    }
  }

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (searchQuery.trim()) {
      handleNavigate(searchQuery)
      setSearchQuery('')
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', backgroundColor: 'var(--window-bg)' }}>
      {/* Tabs Strip */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          backgroundColor: 'rgba(0, 0, 0, 0.15)',
          padding: '6px 8px 0 8px',
          gap: '4px',
          overflowX: 'auto',
        }}
      >
        {tabs.map(tab => {
          const isActive = tab.id === activeTabId
          return (
            <div
              key={tab.id}
              onClick={() => {
                setActiveTabId(tab.id)
                setUrlInput(tab.url)
              }}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                padding: '6px 12px',
                minWidth: '140px',
                maxWidth: '200px',
                borderRadius: '6px 6px 0 0',
                backgroundColor: isActive ? 'var(--window-bg)' : 'transparent',
                border: isActive ? '1px solid var(--window-border)' : '1px solid transparent',
                borderBottom: 'none',
                cursor: 'pointer',
                fontSize: '12px',
                color: isActive ? 'var(--text-primary)' : 'var(--text-secondary)',
              }}
            >
              <FluentIcon name="globe" size={13} color="var(--win-accent)" />
              <span
                style={{
                  flex: 1,
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                {tab.title}
              </span>
              {tabs.length > 1 && (
                <button
                  onClick={e => handleCloseTab(e, tab.id)}
                  style={{
                    border: 'none',
                    background: 'transparent',
                    color: 'var(--text-tertiary)',
                    borderRadius: '50%',
                    padding: '2px',
                    display: 'flex',
                    cursor: 'pointer',
                  }}
                  onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
                  onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
                >
                  <FluentIcon name="x" size={11} />
                </button>
              )}
            </div>
          )
        })}

        <button
          onClick={handleNewTab}
          title="Nouvel onglet"
          style={{
            border: 'none',
            background: 'transparent',
            color: 'var(--text-primary)',
            padding: '6px',
            borderRadius: '4px',
            cursor: 'pointer',
            display: 'flex',
          }}
          onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
          onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
        >
          <FluentIcon name="plus" size={14} />
        </button>
      </div>

      {/* Navigation Toolbar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '8px',
          padding: '6px 12px',
          borderBottom: '1px solid var(--window-border)',
          backgroundColor: 'var(--window-bg)',
        }}
      >
        <div style={{ display: 'flex', gap: '2px' }}>
          <button
            onClick={goBack}
            disabled={historyIndex <= 0}
            style={{
              padding: '6px',
              border: 'none',
              background: 'transparent',
              color: historyIndex <= 0 ? 'var(--text-tertiary)' : 'var(--text-secondary)',
              borderRadius: '4px',
              cursor: historyIndex <= 0 ? 'default' : 'pointer',
            }}
          >
            <FluentIcon name="arrow-left" size={14} />
          </button>
          <button
            onClick={goForward}
            disabled={historyIndex >= history.length - 1}
            style={{
              padding: '6px',
              border: 'none',
              background: 'transparent',
              color: historyIndex >= history.length - 1 ? 'var(--text-tertiary)' : 'var(--text-secondary)',
              borderRadius: '4px',
              cursor: historyIndex >= history.length - 1 ? 'default' : 'pointer',
            }}
          >
            <FluentIcon name="arrow-right" size={14} />
          </button>
          <button
            onClick={() => handleNavigate(urlInput)}
            style={{
              padding: '6px',
              border: 'none',
              background: 'transparent',
              color: 'var(--text-secondary)',
              borderRadius: '4px',
              cursor: 'pointer',
            }}
          >
            <FluentIcon name="refresh" size={13} />
          </button>
        </div>

        {/* Address Bar */}
        <form
          onSubmit={e => {
            e.preventDefault()
            handleNavigate(urlInput)
          }}
          style={{
            flex: 1,
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '4px 12px',
            borderRadius: '16px',
            backgroundColor: 'var(--flyout-card)',
            border: '1px solid var(--window-border)',
          }}
        >
          <FluentIcon name="lock" size={12} color="var(--text-tertiary)" />
          <input
            type="text"
            value={urlInput}
            onChange={e => setUrlInput(e.target.value)}
            style={{
              width: '100%',
              border: 'none',
              background: 'transparent',
              outline: 'none',
              fontSize: '12.5px',
              color: 'var(--text-primary)',
            }}
          />
        </form>

        <div style={{ display: 'flex', gap: '6px' }}>
          <button
            title="Copilot"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '4px',
              padding: '4px 10px',
              borderRadius: '14px',
              border: '1px solid var(--win-accent)',
              backgroundColor: 'var(--win-accent-light)',
              color: 'var(--win-accent)',
              fontSize: '12px',
              fontWeight: 500,
              cursor: 'pointer',
            }}
          >
            <FluentIcon name="sparkles" size={13} />
            <span>Copilot</span>
          </button>
        </div>
      </div>

      {/* Bookmarks Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '12px',
          padding: '4px 16px',
          borderBottom: '1px solid var(--window-border)',
          fontSize: '11.5px',
          color: 'var(--text-secondary)',
          backgroundColor: 'rgba(255, 255, 255, 0.01)',
        }}
      >
        {[
          { name: 'Bing', url: 'https://www.bing.com' },
          { name: 'GitHub', url: 'https://github.com' },
          { name: 'YouTube', url: 'https://youtube.com' },
          { name: 'Wikipédia', url: 'https://fr.wikipedia.org' },
          { name: 'Reddit', url: 'https://reddit.com' },
        ].map(bm => (
          <div
            key={bm.name}
            onClick={() => handleNavigate(bm.url)}
            style={{ display: 'flex', alignItems: 'center', gap: '4px', cursor: 'pointer' }}
            onMouseEnter={e => (e.currentTarget.style.color = 'var(--text-primary)')}
            onMouseLeave={e => (e.currentTarget.style.color = 'var(--text-secondary)')}
          >
            <FluentIcon name="globe" size={12} />
            <span>{bm.name}</span>
          </div>
        ))}
      </div>

      {/* Main Page Area */}
      <div
        style={{
          flex: 1,
          overflow: 'auto',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          padding: '30px 20px',
          backgroundColor: 'var(--win-bg)',
        }}
      >
        {searchResults ? (
          /* Search Results View */
          <div style={{ maxWidth: '720px', width: '100%' }}>
            <h3 style={{ marginBottom: '16px', fontSize: '18px' }}>
              Résultats de recherche pour « {searchResults} »
            </h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
              {[
                {
                  title: `${searchResults} - Guide et Actualités Microsoft`,
                  url: `https://www.bing.com/news?q=${encodeURIComponent(searchResults)}`,
                  snippet: `Découvrez les dernières informations complètes et ressources officielles sur ${searchResults}.`,
                },
                {
                  title: `${searchResults} - Wikipédia, l'encyclopédie libre`,
                  url: `https://fr.wikipedia.org/wiki/${encodeURIComponent(searchResults)}`,
                  snippet: `${searchResults} est un sujet majeur documenté avec historique, références, contextes et analyses détaillées.`,
                },
                {
                  title: `Téléchargements et outils pour ${searchResults}`,
                  url: `https://learn.microsoft.com/search/?terms=${encodeURIComponent(searchResults)}`,
                  snippet: `Guides techniques, documentation pour développeurs et bonnes pratiques sur la plateforme Windows 11.`,
                },
              ].map((res, i) => (
                <div
                  key={i}
                  style={{
                    padding: '14px',
                    borderRadius: '8px',
                    backgroundColor: 'var(--flyout-card)',
                    border: '1px solid var(--window-border)',
                  }}
                >
                  <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '4px' }}>
                    {res.url}
                  </div>
                  <h4 style={{ fontSize: '15px', color: 'var(--win-accent)', marginBottom: '6px', cursor: 'pointer' }}>
                    {res.title}
                  </h4>
                  <p style={{ fontSize: '13px', color: 'var(--text-secondary)', lineHeight: '1.4' }}>
                    {res.snippet}
                  </p>
                </div>
              ))}
            </div>
          </div>
        ) : (
          /* Default Bing Home Page */
          <div
            style={{
              maxWidth: '680px',
              width: '100%',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              marginTop: '40px',
            }}
          >
            {/* Bing Logo */}
            <h1
              style={{
                fontSize: '44px',
                fontWeight: 700,
                letterSpacing: '-1px',
                marginBottom: '24px',
                background: 'linear-gradient(135deg, #0078d4 0%, #00bcf2 100%)',
                WebkitBackgroundClip: 'text',
                WebkitTextFillColor: 'transparent',
              }}
            >
              Microsoft Bing
            </h1>

            {/* Central Search Form */}
            <form
              onSubmit={handleSearchSubmit}
              style={{
                width: '100%',
                display: 'flex',
                alignItems: 'center',
                backgroundColor: 'var(--flyout-bg)',
                borderRadius: '24px',
                padding: '10px 18px',
                border: '1px solid var(--flyout-border)',
                boxShadow: '0 8px 24px rgba(0,0,0,0.3)',
                marginBottom: '32px',
              }}
            >
              <FluentIcon name="search" size={18} color="var(--win-accent)" />
              <input
                type="text"
                placeholder="Rechercher sur le web ou poser une question..."
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
                style={{
                  flex: 1,
                  border: 'none',
                  background: 'transparent',
                  outline: 'none',
                  marginLeft: '12px',
                  fontSize: '14px',
                  color: 'var(--text-primary)',
                }}
              />
              <button
                type="submit"
                style={{
                  border: 'none',
                  backgroundColor: 'var(--win-accent)',
                  color: '#ffffff',
                  padding: '6px 14px',
                  borderRadius: '16px',
                  fontSize: '12px',
                  fontWeight: 500,
                  cursor: 'pointer',
                }}
              >
                Rechercher
              </button>
            </form>

            {/* Quick tiles */}
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(4, 1fr)',
                gap: '16px',
                width: '100%',
              }}
            >
              {[
                { name: 'GitHub', icon: 'code', desc: 'Développement' },
                { name: 'Actualités', icon: 'globe', desc: 'En direct' },
                { name: 'Météo', icon: 'sun', desc: 'Prévisions' },
                { name: 'Office 365', icon: 'file-text', desc: 'Productivité' },
              ].map(tile => (
                <div
                  key={tile.name}
                  onClick={() => handleNavigate(`https://www.bing.com/search?q=${tile.name}`)}
                  style={{
                    display: 'flex',
                    flexDirection: 'column',
                    alignItems: 'center',
                    padding: '16px 10px',
                    borderRadius: '10px',
                    backgroundColor: 'var(--flyout-card)',
                    border: '1px solid var(--window-border)',
                    cursor: 'pointer',
                    transition: 'transform 0.15s, background 0.15s',
                  }}
                  onMouseEnter={e => {
                    e.currentTarget.style.backgroundColor = 'var(--flyout-card-hover)'
                    e.currentTarget.style.transform = 'translateY(-2px)'
                  }}
                  onMouseLeave={e => {
                    e.currentTarget.style.backgroundColor = 'var(--flyout-card)'
                    e.currentTarget.style.transform = 'translateY(0)'
                  }}
                >
                  <FluentIcon name={tile.icon} size={24} color="var(--win-accent)" />
                  <span style={{ fontSize: '13px', fontWeight: 500, marginTop: '8px' }}>
                    {tile.name}
                  </span>
                  <span style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>{tile.desc}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
