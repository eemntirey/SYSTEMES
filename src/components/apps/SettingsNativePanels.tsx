import React, { useEffect, useState } from 'react'
import {
  readStorageVolumes,
  readDisplayLayout,
  readInstalledApps,
  type StorageSnapshot,
  type DisplaySnapshot,
  type AppsSnapshot,
} from '../../native/p2'

const muted: React.CSSProperties = { color: 'var(--text-secondary)', fontSize: 12, lineHeight: 1.45 }
const panel: React.CSSProperties = {
  background: 'var(--flyout-card)',
  border: '1px solid var(--window-border)',
  borderRadius: 8,
  overflow: 'hidden',
  marginBottom: 18,
}
const button: React.CSSProperties = {
  border: '1px solid var(--window-border)',
  borderRadius: 6,
  padding: '7px 12px',
  background: 'var(--window-bg)',
  color: 'var(--text-primary)',
  cursor: 'pointer',
  fontSize: 12,
}

const Row: React.FC<{
  title: string
  description?: string
  control?: React.ReactNode
  last?: boolean
}> = ({ title, description, control, last }) => (
  <div
    style={{
      minHeight: 56,
      padding: '12px 16px',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: 16,
      borderBottom: last ? 0 : '1px solid var(--window-border)',
    }}
  >
    <div style={{ minWidth: 0 }}>
      <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 3 }}>{title}</div>
      {description && <div style={muted}>{description}</div>}
    </div>
    {control && <div style={{ flexShrink: 0 }}>{control}</div>}
  </div>
)

const Group: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <>
    <h2 style={{ fontSize: 15, fontWeight: 650, margin: '25px 0 10px' }}>{title}</h2>
    <div style={panel}>{children}</div>
  </>
)

function formatBytes(n: number) {
  if (!Number.isFinite(n) || n <= 0) return '0 o'
  const units = ['o', 'Ko', 'Mo', 'Go', 'To']
  let v = n
  let i = 0
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024
    i++
  }
  return `${v < 10 && i > 0 ? v.toFixed(1) : Math.round(v)} ${units[i]}`
}

export const DisplayNativePanel: React.FC = () => {
  const [display, setDisplay] = useState<DisplaySnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const layout = await readDisplayLayout()
        if (!cancelled) {
          setDisplay(layout)
          setError(null)
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Group title="Écrans Windows (natif)">
      {error && <Row title="Indisponible" description={error} last />}
      {!error && !display && <Row title="Chargement…" description="Lecture EnumDisplayMonitors…" last />}
      {display && (
        <>
          <Row title="Écran principal" description={display.resolution} />
          <Row title="Nombre d’écrans" description={String(display.monitorCount)} />
          <Row
            title="Bureau virtuel"
            description={`${display.virtualScreen?.width ?? '—'} × ${display.virtualScreen?.height ?? '—'}`}
            last={!display.monitors?.length}
          />
          {(display.monitors ?? []).map((m, i, all) => (
            <Row
              key={m.device || i}
              title={m.isPrimary ? `Écran ${i + 1} (principal)` : `Écran ${i + 1}`}
              description={`${m.resolution} · ${m.device || 'sans nom'} · ${m.bounds.width}×${m.bounds.height} @ (${m.bounds.x},${m.bounds.y})`}
              last={i === all.length - 1}
            />
          ))}
        </>
      )}
    </Group>
  )
}

export const InstalledAppsNativePanel: React.FC = () => {
  const [apps, setApps] = useState<AppsSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      setLoading(true)
      try {
        const snap = await readInstalledApps(250)
        if (!cancelled) {
          setApps(snap)
          setError(null)
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Group title="Applications installées sur Windows">
      {loading && <Row title="Chargement…" description="Lecture du registre Uninstall…" last />}
      {error && !loading && <Row title="Indisponible" description={error} last />}
      {!loading && !error && apps && (
        <>
          <Row
            title="Inventaire"
            description={`${apps.count} application(s)${apps.truncated ? ` (tronqué, ${apps.totalFound} trouvées)` : ''} · ${apps.source}`}
          />
          {apps.apps.slice(0, 40).map((app, i, all) => (
            <Row
              key={app.id}
              title={app.name}
              description={
                [app.version && `v${app.version}`, app.publisher, app.installDate].filter(Boolean).join(' · ') ||
                'Sans détails'
              }
              control={<span style={muted}>{app.hasUninstaller ? 'Désinstallable' : ''}</span>}
              last={i === all.length - 1}
            />
          ))}
          {apps.apps.length > 40 && (
            <Row title="…" description={`${apps.apps.length - 40} autres non affichées ici.`} last />
          )}
        </>
      )}
      {!loading && !error && !apps && (
        <Row title="Aucune donnée" description="Ouvrez cette section depuis l’hôte Windows." last />
      )}
    </Group>
  )
}

export const StorageNativePanel: React.FC = () => {
  const [storage, setStorage] = useState<StorageSnapshot | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const snap = await readStorageVolumes()
        if (!cancelled) {
          setStorage(snap)
          setError(null)
        }
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      }
    })()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <Group title="Stockage Windows (natif)">
      {error && <Row title="Indisponible" description={error} last />}
      {!error && !storage && <Row title="Chargement…" description="Lecture des volumes…" last />}
      {storage && (
        <>
          <Row title="Disques physiques" description={`${storage.diskCount} disque(s) · ${storage.source}`} />
          {(storage.disks ?? []).map((d, i) => (
            <Row
              key={`disk-${i}`}
              title={d.model || `Disque ${i + 1}`}
              description={[d.interfaceType, d.mediaType, formatBytes(d.sizeBytes), d.status]
                .filter(Boolean)
                .join(' · ')}
            />
          ))}
          {(storage.volumes ?? []).map((v, i, all) => (
            <Row
              key={v.name}
              title={`${v.name} ${v.label ? `(${v.label})` : ''}`.trim()}
              description={
                v.isReady
                  ? `${v.format || '—'} · ${formatBytes(v.usedBytes)} utilisés / ${formatBytes(v.totalBytes)} · ${v.freePercent}% libre`
                  : `${v.driveType} · non prêt`
              }
              last={i === all.length - 1}
            />
          ))}
        </>
      )}
    </Group>
  )
}

export const DefaultAppsOpenRow: React.FC<{ onOpen: () => void }> = ({ onOpen }) => (
  <Group title="Applications par défaut">
    <Row
      title="Choisir les applications par défaut"
      description="Ouvre les paramètres Windows réels."
      control={
        <button type="button" style={button} onClick={onOpen}>
          Ouvrir
        </button>
      }
      last
    />
  </Group>
)
