import React, { useEffect, useMemo, useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { WALLPAPERS } from '../../data/constants'
import { NativeBridge } from '../../services/NativeBridge'
import { readDisplayLayout, readInstalledApps, readStorageVolumes, type AppsSnapshot, type DisplaySnapshot, type StorageSnapshot } from '../../native/p2'

type SectionId = 'home' | 'system' | 'bluetooth' | 'network' | 'personalization' | 'apps' | 'accounts' | 'time' | 'gaming' | 'accessibility' | 'privacy' | 'update' | 'storage' | 'about'

type WindowsUpdateResult = {
  success?: boolean
  serviceRunning?: boolean | null
  updatesAvailable?: boolean | null
  updateCount?: number | null
  updates?: Array<{ title: string; kbArticleIds?: string[]; isDownloaded?: boolean; isMandatory?: boolean }>
  truncated?: boolean
  rebootRequired?: boolean | null
  checkedAtUtc?: string
  source?: string
  error?: string
  note?: string
}
type NavItem = { id: SectionId; label: string; description: string; icon: string }

const NAV: NavItem[] = [
  { id: 'home', label: 'Accueil', description: 'Vue d’ensemble', icon: 'home' },
  { id: 'system', label: 'Système', description: 'Affichage, son, notifications, alimentation', icon: 'monitor' },
  { id: 'storage', label: 'Stockage', description: 'Volumes, disques et espace disponible', icon: 'hard-drive' },
  { id: 'bluetooth', label: 'Bluetooth et appareils', description: 'Bluetooth et périphériques', icon: 'bluetooth' },
  { id: 'network', label: 'Réseau et Internet', description: 'Wi-Fi, mode avion et partage de connexion', icon: 'wifi' },
  { id: 'personalization', label: 'Personnalisation', description: 'Arrière-plan, couleurs, écran de verrouillage', icon: 'palette' },
  { id: 'apps', label: 'Applications', description: 'Applications intégrées et paramètres', icon: 'grid' },
  { id: 'accounts', label: 'Comptes', description: 'Profil et synchronisation', icon: 'user' },
  { id: 'time', label: 'Heure et langue', description: 'Date, heure et région', icon: 'clock' },
  { id: 'gaming', label: 'Jeux', description: 'Mode Jeu et captures', icon: 'gamepad-2' },
  { id: 'accessibility', label: 'Accessibilité', description: 'Vision, audition et interaction', icon: 'accessibility' },
  { id: 'privacy', label: 'Confidentialité et sécurité', description: 'Autorisations et données', icon: 'shield-check' },
  { id: 'update', label: 'Windows Update', description: 'Préférences de mise à jour', icon: 'refresh-cw' },
  { id: 'about', label: 'Informations système', description: 'Version et environnement', icon: 'info' },
]
const ACCENTS = ['#0078d4','#00a2ed','#00b7c3','#10893e','#8764b8','#744da9','#d13438','#ca5010','#e3008c','#ff4b82','#ffb900','#64748b']

const shell: React.CSSProperties = { display: 'flex', width: '100%', height: '100%', minHeight: 0, overflow: 'hidden', background: 'var(--window-bg)', color: 'var(--text-primary)', fontFamily: 'var(--font-segoe, Segoe UI, sans-serif)' }
const sidebar: React.CSSProperties = { width: 270, minWidth: 220, flexShrink: 0, display: 'flex', flexDirection: 'column', gap: 12, padding: '20px 12px 14px', background: 'var(--flyout-card)', borderRight: '1px solid var(--window-border)', overflowY: 'auto' }
const content: React.CSSProperties = { flex: 1, minWidth: 0, overflowY: 'auto', padding: '30px clamp(18px, 4vw, 56px) 48px' }
const panel: React.CSSProperties = { background: 'var(--flyout-card)', border: '1px solid var(--window-border)', borderRadius: 8, overflow: 'hidden', marginBottom: 18 }
const muted: React.CSSProperties = { color: 'var(--text-secondary)', fontSize: 12, lineHeight: 1.45 }
const button: React.CSSProperties = { border: '1px solid var(--window-border)', borderRadius: 6, padding: '7px 12px', background: 'var(--window-bg)', color: 'var(--text-primary)', cursor: 'pointer', fontSize: 12 }

const Toggle: React.FC<{ value: boolean; onChange: () => void; label: string }> = ({ value, onChange, label }) => (
  <button type="button" role="switch" aria-checked={value} aria-label={label} onClick={onChange} style={{ width: 40, height: 22, borderRadius: 12, border: value ? '1px solid var(--win-accent)' : '1px solid var(--text-tertiary)', background: value ? 'var(--win-accent)' : 'transparent', padding: 2, cursor: 'pointer', display: 'flex', alignItems: 'center', justifyContent: value ? 'flex-end' : 'flex-start', flexShrink: 0 }}>
    <span style={{ width: 14, height: 14, borderRadius: '50%', background: value ? '#fff' : 'var(--text-secondary)' }} />
  </button>
)
const Row: React.FC<{ title: string; description?: string; control?: React.ReactNode; icon?: string; last?: boolean }> = ({ title, description, control, icon, last }) => (
  <div style={{ minHeight: 64, padding: '12px 16px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 16, borderBottom: last ? 0 : '1px solid var(--window-border)' }}>
    <div style={{ display: 'flex', alignItems: 'center', gap: 12, minWidth: 0 }}>
      {icon && <FluentIcon name={icon} size={18} color="var(--text-secondary)" />}
      <div style={{ minWidth: 0 }}><div style={{ fontSize: 13, fontWeight: 600, marginBottom: 3 }}>{title}</div>{description && <div style={muted}>{description}</div>}</div>
    </div>
    {control && <div style={{ flexShrink: 0 }}>{control}</div>}
  </div>
)
const Group: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => <><h2 style={{ fontSize: 15, fontWeight: 650, margin: '25px 0 10px' }}>{title}</h2><div style={panel}>{children}</div></>
const Range: React.FC<{ value: number; onChange: (value: number) => void; label: string; min?: number; max?: number }> = ({ value, onChange, label, min = 0, max = 100 }) => <div style={{ display: 'flex', alignItems: 'center', gap: 9 }}><input aria-label={label} type="range" min={min} max={max} value={value} onChange={e => onChange(Number(e.target.value))} style={{ width: 125, accentColor: 'var(--win-accent)' }} /><span style={{ ...muted, width: 35, textAlign: 'right' }}>{value}%</span></div>

const formatStorageBytes = (bytes: number) => {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 Gio'
  const units = ['o', 'Kio', 'Mio', 'Gio', 'Tio']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) { value /= 1024; unit++ }
  return `${value.toLocaleString('fr-FR', { maximumFractionDigits: 1 })} ${units[unit]}`
}

export const SettingsApp: React.FC = () => {
  const s = useOS()
  const [section, setSection] = useState<SectionId>('home')
  const [query, setQuery] = useState('')
  const [region, setRegion] = useState('MG')
  const [language, setLanguage] = useState('fr')
  const [updateInfo, setUpdateInfo] = useState('Aucune recherche Windows Update effectuée dans cette session.')
  const [updateResult, setUpdateResult] = useState<WindowsUpdateResult | null>(null)
  const [checkingUpdates, setCheckingUpdates] = useState(false)
  const [appsSnapshot, setAppsSnapshot] = useState<AppsSnapshot | null>(null)
  const [appsError, setAppsError] = useState<string | null>(null)
  const [appsLoading, setAppsLoading] = useState(false)
  const [storageSnapshot, setStorageSnapshot] = useState<StorageSnapshot | null>(null)
  const [storageError, setStorageError] = useState<string | null>(null)
  const [storageLoading, setStorageLoading] = useState(false)
  const [displaySnapshot, setDisplaySnapshot] = useState<DisplaySnapshot | null>(null)
  const [displayError, setDisplayError] = useState<string | null>(null)
  const [displayLoading, setDisplayLoading] = useState(false)

  const filtered = useMemo(() => {
    const q = query.trim().toLocaleLowerCase('fr')
    return q ? NAV.filter(item => `${item.label} ${item.description}`.toLocaleLowerCase('fr').includes(q)) : NAV
  }, [query])
  const current = NAV.find(item => item.id === section) ?? NAV[0]

  useEffect(() => {
    let active = true
    if (section === 'apps') {
      setAppsLoading(true)
      setAppsError(null)
      void readInstalledApps().then(value => { if (active) setAppsSnapshot(value) })
        .catch(cause => { if (active) setAppsError(cause instanceof Error ? cause.message : String(cause)) })
        .finally(() => { if (active) setAppsLoading(false) })
    }
    if (section === 'storage') {
      setStorageLoading(true)
      setStorageError(null)
      void readStorageVolumes().then(value => { if (active) setStorageSnapshot(value) })
        .catch(cause => { if (active) setStorageError(cause instanceof Error ? cause.message : String(cause)) })
        .finally(() => { if (active) setStorageLoading(false) })
    }
    if (section === 'system') {
      setDisplayLoading(true)
      setDisplayError(null)
      void readDisplayLayout().then(value => { if (active) setDisplaySnapshot(value) })
        .catch(cause => { if (active) setDisplayError(cause instanceof Error ? cause.message : String(cause)) })
        .finally(() => { if (active) setDisplayLoading(false) })
    }
    return () => { active = false }
  }, [section])
  const checkWindowsUpdates = async () => {
    if (checkingUpdates) return
    setCheckingUpdates(true)
    setUpdateInfo('Interrogation de Windows Update Agent…')
    try {
      const result = await NativeBridge.checkWindowsUpdate() as WindowsUpdateResult
      setUpdateResult(result)
      if (result.success && result.updatesAvailable === true) {
        setUpdateInfo(`${result.updateCount ?? result.updates?.length ?? 0} mise(s) à jour proposée(s) par Windows Update. Aucun téléchargement ni installation n’a été lancé.`)
      } else if (result.success && result.updatesAvailable === false) {
        setUpdateInfo('Windows Update ne signale aucune mise à jour logicielle disponible.')
      } else {
        setUpdateInfo(result.error || 'Windows Update n’a pas pu terminer la recherche.')
      }
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause)
      setUpdateResult({ success: false, error: message, note: 'Aucune modification Windows n’a été effectuée.' })
      setUpdateInfo(message)
    } finally {
      setCheckingUpdates(false)
    }
  }
  const go = (id: SectionId) => { setSection(id); setQuery('') }
  const toggle = (value: boolean, action: () => void, label: string) => <Toggle value={value} onChange={action} label={label} />
  const choose = (value: string, onChange: (value: string) => void, options: Array<[string, string]>, label: string) => (
    <select aria-label={label} value={value} onChange={e => onChange(e.target.value)} style={{ ...button, minWidth: 140 }}>{options.map(([v, labelText]) => <option key={v} value={v}>{labelText}</option>)}</select>
  )

  const renderPage = () => {
    switch (section) {
      case 'home':
        return <>
          <div style={{ ...panel, padding: 20, display: 'flex', alignItems: 'center', gap: 16, flexWrap: 'wrap' }}>
            <div style={{ width: 54, height: 54, borderRadius: '50%', background: 'var(--win-accent)', color: '#fff', display: 'grid', placeItems: 'center', fontSize: 22 }}>U</div>
            <div style={{ flex: 1 }}><div style={{ fontSize: 18, fontWeight: 650 }}>Utilisateur</div><div style={muted}>Compte local · Session SYSTEMES</div></div>
            <span style={{ ...muted, border: '1px solid var(--window-border)', padding: '6px 10px', borderRadius: 6 }}>Profil local</span>
          </div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(190px, 1fr))', gap: 12 }}>
            {[
              { id: 'system' as const, title: 'Écran', detail: `Luminosité : ${s.brightness}%`, icon: 'monitor' },
              { id: 'network' as const, title: 'Réseau', detail: s.wifiEnabled ? 'Wi-Fi activé' : 'Wi-Fi désactivé', icon: 'wifi' },
              { id: 'bluetooth' as const, title: 'Bluetooth', detail: s.bluetoothEnabled ? 'Activé' : 'Désactivé', icon: 'bluetooth' },
              { id: 'personalization' as const, title: 'Personnalisation', detail: s.theme === 'dark' ? 'Mode sombre' : 'Mode clair', icon: 'palette' },
            ].map(card => <button key={card.id} type="button" onClick={() => go(card.id)} style={{ ...panel, margin: 0, padding: 16, textAlign: 'left', cursor: 'pointer', color: 'var(--text-primary)' }}><FluentIcon name={card.icon} size={22} color="var(--win-accent)" /><div style={{ marginTop: 12, fontSize: 14, fontWeight: 650 }}>{card.title}</div><div style={{ ...muted, marginTop: 4 }}>{card.detail}</div></button>)}
          </div>
          <Group title="Paramètres recommandés">
            <Row title="Mode nuit" description="Réduit la lumière bleue de l’écran simulé." control={toggle(s.nightLight, s.toggleNightLight, 'Mode nuit')} />
            <Row title="Économiseur de batterie" description="Préférence d’économie d’énergie." control={toggle(s.batterySaver, s.toggleBatterySaver, 'Économiseur de batterie')} last />
          </Group>
          <Group title="Toutes les catégories">{NAV.filter(item => item.id !== 'home').map((item, i, all) => <Row key={item.id} title={item.label} description={item.description} icon={item.icon} control={<button type="button" style={button} onClick={() => go(item.id)}>Ouvrir</button>} last={i === all.length - 1} />)}</Group>
        </>
      case 'system':
        return <>
          <Group title="Affichage">
            <Row title="Luminosité" description="Luminosité de l’affichage simulé." control={<Range label="Luminosité" value={s.brightness} onChange={s.setBrightness} />} />
            <Row title="Échelle du texte" control={choose(String(s.fontSize), v => s.setFontSize(Number(v)), [['90','90 %'],['100','100 %'],['110','110 %'],['125','125 %'],['150','150 %']], 'Échelle du texte')} />
            <Row title="Résolution" description="Résolution logique de l’environnement." control={choose(s.resolution, s.setResolution, [['1280x720','1280 × 720'],['1366x768','1366 × 768'],['1600x900','1600 × 900'],['1920x1080','1920 × 1080'],['2560x1440','2560 × 1440']], 'Résolution')} />
            <Row title="Fréquence de rafraîchissement" control={choose(s.refreshRate, s.setRefreshRate, [['60Hz','60 Hz'],['75Hz','75 Hz'],['120Hz','120 Hz'],['144Hz','144 Hz']], 'Fréquence')} last />
          </Group>
          <Group title="Écrans connectés">
            {displayLoading && <Row title="Lecture des écrans Windows…" description="Interrogation de l’API User32." />}
            {displayError && <Row title="Inventaire indisponible" description={displayError} />}
            {displaySnapshot && <>
              <Row title={`${displaySnapshot.monitorCount} écran(s) détecté(s)`} description={`Écran principal : ${displaySnapshot.resolution} · Bureau virtuel : ${displaySnapshot.virtualScreen.width} × ${displaySnapshot.virtualScreen.height}`} />
              {displaySnapshot.monitors.map((monitor, index) => <Row
                key={monitor.device || index}
                title={`${monitor.device || `Écran ${index + 1}`} ${monitor.isPrimary ? '· Principal' : ''}`}
                description={`${monitor.resolution} · Position ${monitor.bounds.x}, ${monitor.bounds.y} · Zone de travail ${monitor.workArea.width} × ${monitor.workArea.height}`}
                last={index === displaySnapshot.monitors.length - 1}
              />)}
            </>}
          </Group>
          <Group title="Son">
            <Row title="Volume général" control={<Range label="Volume" value={s.volume} onChange={s.setVolume} />} />
            <Row title="Son" description={s.isMuted ? 'Son coupé' : 'Son activé'} control={<button style={button} onClick={s.toggleMute}>{s.isMuted ? 'Réactiver' : 'Couper le son'}</button>} last />
          </Group>
          <Group title="Notifications et concentration">
            <Row title="Notifications" control={toggle(s.notificationsEnabled, s.toggleNotifications, 'Notifications')} />
            <Row title="Ne pas déranger" control={toggle(s.focusAssist, () => s.setFocusAssist(!s.focusAssist), 'Ne pas déranger')} last />
          </Group>
          <Group title="Alimentation">
            <Row title="Économiseur de batterie" control={toggle(s.batterySaver, s.toggleBatterySaver, 'Économiseur de batterie')} />
            <Row title="Mode nuit" control={toggle(s.nightLight, s.toggleNightLight, 'Mode nuit')} last />
          </Group>
        </>
      case 'bluetooth':
        return <Group title="Appareils">
          <Row title="Bluetooth" description="Active ou désactive le Bluetooth dans SYSTEMES." control={toggle(s.bluetoothEnabled, s.toggleBluetooth, 'Bluetooth')} />
          <Row title="Périphériques associés" description="La gestion des appareils physiques nécessite une intégration système non disponible." control={<span style={muted}>Non connecté au matériel</span>} />
          <Row title="Ajouter un appareil" description="Association d’un appareil physique indisponible dans cette simulation." control={<button style={button} disabled>Indisponible</button>} last />
        </Group>
      case 'network':
        return <Group title="Réseau et Internet">
          <Row title="Wi-Fi" description="État du réseau simulé." control={toggle(s.wifiEnabled, s.toggleWifi, 'Wi-Fi')} />
          <Row title="Mode avion" description="Préférence réseau locale." control={toggle(s.airplaneMode, s.toggleAirplaneMode, 'Mode avion')} />
          <Row title="Point d’accès mobile" description="Fonction de partage simulée, sans connexion réelle." control={toggle(s.mobileHotspot, s.toggleMobileHotspot, 'Point d’accès mobile')} last />
        </Group>
      case 'personalization':
        return <>
          <Group title="Mode et barre des tâches">
            <Row title="Mode d’application" control={choose(s.theme, v => s.setTheme(v as 'dark' | 'light'), [['dark','Sombre'],['light','Clair']], 'Mode d’application')} />
            <Row title="Alignement de la barre des tâches" control={choose(s.taskbarAlignment, v => s.setTaskbarAlignment(v as 'center' | 'left'), [['center','Centré'],['left','À gauche']], 'Alignement')} />
            <Row title="Afficher les libellés du bureau" control={toggle(s.showDesktopLabels, () => s.setShowDesktopLabels(!s.showDesktopLabels), 'Libellés du bureau')} last />
          </Group>
          <Group title="Arrière-plan"><div style={{ padding: 16, display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(110px, 1fr))', gap: 12 }}>{WALLPAPERS.map(wp => <button key={wp.id} type="button" aria-label={`Fond d’écran ${wp.id}`} onClick={() => s.setWallpaper(wp)} style={{ height: 76, borderRadius: 6, border: s.wallpaper.id === wp.id ? '3px solid var(--win-accent)' : '1px solid var(--window-border)', backgroundImage: `url("${wp.url}")`, backgroundSize: 'cover', backgroundPosition: 'center', cursor: 'pointer' }} />)}</div></Group>
          <Group title="Couleur d’accentuation"><div style={{ padding: 16, display: 'flex', flexWrap: 'wrap', gap: 12 }}>{ACCENTS.map(color => <button key={color} type="button" aria-label={`Couleur ${color}`} aria-pressed={s.accentColor.toLowerCase() === color} onClick={() => { s.setAccentColor(color); document.documentElement.style.setProperty('--win-accent', color) }} style={{ width: 30, height: 30, borderRadius: '50%', border: s.accentColor.toLowerCase() === color ? '3px solid var(--text-primary)' : '2px solid transparent', background: color, cursor: 'pointer' }} />)}</div></Group>
          <Group title="Écran de verrouillage">
            <Row title="Arrière-plan de l’écran de verrouillage" control={toggle(s.lockScreenBackground, s.toggleLockScreenBackground, 'Arrière-plan de verrouillage')} />
            <Row title="État des applications" control={toggle(s.lockScreenAppStatus, s.toggleLockScreenAppStatus, 'État des applications')} last />
          </Group>
        </>
      case 'apps':
        return <>
          <Group title="Applications intégrées">
            {['Explorateur de fichiers','Navigateur web','Bloc-notes','Terminal','Calculatrice','Paint','Photos','Paramètres'].map((name, i, all) => <Row key={name} title={name} description="Application disponible dans le bureau SYSTEMES." control={<span style={muted}>Intégrée</span>} last={i === all.length - 1} />)}
          </Group>
          <Group title="Applications Windows installées">
            {appsLoading && <Row title="Inventaire en cours…" description="Lecture des entrées de désinstallation du Registre Windows." />}
            {appsError && <Row title="Inventaire indisponible" description={appsError} />}
            {appsSnapshot && <>
              <Row title={`${appsSnapshot.totalFound} application(s) détectée(s)`} description={`${appsSnapshot.source} · ${appsSnapshot.note}`} />
              {appsSnapshot.apps.map((app, index) => <Row
                key={app.id}
                title={app.name}
                description={[app.version ? `Version ${app.version}` : '', app.publisher, app.installDate ? `Installée le ${app.installDate}` : ''].filter(Boolean).join(' · ') || 'Informations éditeur non fournies'}
                control={<span style={muted}>{app.hasUninstaller ? 'Désinstallateur détecté' : 'Inventaire seul'}</span>}
                last={index === appsSnapshot.apps.length - 1}
              />)}
              {appsSnapshot.truncated && <Row title="Liste limitée" description={`Affichage de ${appsSnapshot.count} applications sur ${appsSnapshot.totalFound} détectées.`} last />}
            </>}
          </Group>
          <Group title="Applications par défaut"><Row title="Choisir les applications par défaut" description="L’association par type de fichier n’est pas encore reliée à un gestionnaire." control={<span style={muted}>Non configuré</span>} last /></Group>
        </>
      case 'storage':
        return <>
          <Group title="Volumes Windows">
            {storageLoading && <Row title="Lecture des volumes…" description="Interrogation de DriveInfo." />}
            {storageError && <Row title="Inventaire indisponible" description={storageError} />}
            {storageSnapshot && storageSnapshot.volumes.map((volume, index) => <Row
              key={volume.name}
              title={`${volume.name} ${volume.label ? `· ${volume.label}` : ''}`}
              description={volume.isReady
                ? `${volume.driveType} · ${volume.format || 'Format inconnu'} · ${formatStorageBytes(volume.freeBytes)} libres sur ${formatStorageBytes(volume.totalBytes)}`
                : `${volume.driveType} · Volume non prêt ou inaccessible`}
              control={volume.isReady ? <div style={{ width: 110 }}>
                <div style={{ fontSize: 11, color: 'var(--text-secondary)', textAlign: 'right' }}>{Math.max(0, 100 - volume.freePercent).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} % utilisé</div>
                <div style={{ height: 6, background: 'var(--window-border)', borderRadius: 6, overflow: 'hidden' }}><div style={{ height: '100%', width: `${Math.max(0, Math.min(100, 100 - volume.freePercent))}%`, background: 'var(--win-accent)' }} /></div>
              </div> : <span style={muted}>Indisponible</span>}
              last={index === storageSnapshot.volumes.length - 1 && storageSnapshot.disks.length === 0}
            />)}
          </Group>
          <Group title="Disques physiques">
            {storageSnapshot?.disks.map((disk, index) => <Row
              key={`${disk.model}-${index}`}
              title={disk.model}
              description={[`${formatStorageBytes(disk.sizeBytes)}`, disk.interfaceType, disk.mediaType, disk.status].filter(Boolean).join(' · ')}
              last={index === storageSnapshot.disks.length - 1}
            />)}
            {storageSnapshot && <Row title={`${storageSnapshot.diskCount} disque(s) physique(s)`} description={`${storageSnapshot.source} · Inventaire en lecture seule ; aucune opération sur les partitions.`} last />}
          </Group>
        </>      case 'accounts':
        return <>
          <div style={{ ...panel, padding: 20, display: 'flex', alignItems: 'center', gap: 16 }}><div style={{ width: 56, height: 56, borderRadius: '50%', background: 'var(--win-accent)', color: '#fff', display: 'grid', placeItems: 'center', fontSize: 22 }}>U</div><div><div style={{ fontSize: 17, fontWeight: 650 }}>Utilisateur</div><div style={muted}>Compte local · Session actuelle</div></div></div>
          <Group title="Synchronisation"><Row title="Synchroniser les paramètres" description="Préférence locale ; aucun compte cloud n’est connecté." control={toggle(s.syncSettings, s.toggleSyncSettings, 'Synchronisation')} last /></Group>
        </>
      case 'time':
        return <>
          <div style={{ ...panel, padding: 22, textAlign: 'center' }}><div style={{ fontSize: 42, fontWeight: 300, fontVariantNumeric: 'tabular-nums' }}>{new Date().toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}</div><div style={muted}>{new Date().toLocaleDateString('fr-FR', { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' })}</div></div>
          <Group title="Date et heure">
            <Row title="Définir l’heure automatiquement" control={toggle(s.setTimeAutomatically, s.toggleSetTimeAutomatically, 'Heure automatique')} />
            <Row title="Définir le fuseau horaire automatiquement" control={toggle(s.setTimeZoneAutomatically, s.toggleSetTimeZoneAutomatically, 'Fuseau horaire automatique')} last />
          </Group>
          <Group title="Langue et région">
            <Row title="Pays ou région" control={choose(region, setRegion, [['MG','Madagascar'],['FR','France'],['US','États-Unis'],['GB','Royaume-Uni'],['DE','Allemagne']], 'Pays ou région')} />
            <Row title="Langue d’affichage" control={choose(language, setLanguage, [['fr','Français'],['en','English']], 'Langue')} last />
          </Group>
        </>
      case 'gaming':
        return <Group title="Jeux">
          <Row title="Xbox Game Bar" control={toggle(s.gameBarEnabled, s.toggleGameBar, 'Xbox Game Bar')} />
          <Row title="Mode Jeu" description="Préférence d’optimisation de l’environnement simulé." control={toggle(s.gameModeEnabled, s.toggleGameMode, 'Mode Jeu')} />
          <Row title="Enregistrement audio" control={toggle(s.audioRecordingEnabled, s.toggleAudioRecording, 'Enregistrement audio')} />
          <Row title="Planification GPU matérielle" description="Préférence enregistrée, sans effet sur le GPU réel." control={toggle(s.hardwareGpuScheduling, s.toggleHardwareGpuScheduling, 'Planification GPU')} />
          <Row title="Taux de rafraîchissement variable" control={toggle(s.vrrEnabled, s.toggleVRR, 'VRR')} last />
        </Group>
      case 'accessibility':
        return <>
          <Group title="Vision">
            <Row title="Taille du texte" control={<Range label="Taille du texte" min={90} max={150} value={s.fontSize} onChange={s.setFontSize} />} />
            <Row title="Loupe" control={toggle(s.magnifierEnabled, s.toggleMagnifier, 'Loupe')} />
            <Row title="Filtres de couleur" control={toggle(s.colorFilterEnabled, s.toggleColorFilter, 'Filtres de couleur')} />
            <Row title="Contraste élevé" control={toggle(s.highContrastEnabled, s.toggleHighContrast, 'Contraste élevé')} last />
          </Group>
          <Group title="Audition et interaction">
            <Row title="Sous-titres" control={toggle(s.captionsEnabled, s.toggleCaptions, 'Sous-titres')} />
            <Row title="Alertes audio" control={toggle(s.audioAlertsEnabled, s.toggleAudioAlerts, 'Alertes audio')} />
            <Row title="Accès vocal" control={toggle(s.voiceAccessEnabled, s.toggleVoiceAccess, 'Accès vocal')} last />
          </Group>
        </>
      case 'privacy':
        return <>
          <Group title="Autorisations des applications">
            <Row title="Localisation" description="Préférence de permission dans la simulation." control={toggle(s.locationEnabled, () => s.setLocationEnabled(!s.locationEnabled), 'Localisation')} />
            <Row title="Caméra" control={toggle(s.cameraEnabled, () => s.setCameraEnabled(!s.cameraEnabled), 'Caméra')} />
            <Row title="Microphone" control={toggle(s.micEnabled, () => s.setMicEnabled(!s.micEnabled), 'Microphone')} last />
          </Group>
          <Group title="Données et diagnostics">
            <Row title="Données de diagnostic" description="Aucune donnée n’est transmise depuis cet écran." control={toggle(s.diagnosticData, () => s.setDiagnosticData(!s.diagnosticData), 'Données de diagnostic')} />
            <Row title="Expériences personnalisées" control={toggle(s.personalizedExperiences, s.togglePersonalizedExperiences, 'Expériences personnalisées')} />
            <Row title="Historique des activités" control={toggle(s.activityHistory, s.toggleActivityHistory, 'Historique des activités')} last />
          </Group>
        </>
      case 'update':
        return <>
          <div style={{ ...panel, padding: 20 }}>
            <div style={{ display: 'flex', gap: 12, alignItems: 'center' }}>
              <FluentIcon name="info" size={24} color="var(--win-accent)" />
              <div>
                <div style={{ fontWeight: 650, fontSize: 15 }}>Windows Update</div>
                <div style={muted}>Recherche réelle via Windows Update Agent. SYSTEMES ne télécharge et n’installe rien.</div>
              </div>
            </div>
            <p style={{ ...muted, margin: '14px 0 0' }}>{updateInfo}</p>
            {updateResult && <div style={{ marginTop: 12, fontSize: 12, color: 'var(--text-secondary)' }}>
              <div>Service Windows Update : {updateResult.serviceRunning == null ? 'État indisponible' : updateResult.serviceRunning ? 'En cours d’exécution' : 'Arrêté'}</div>
              {updateResult.success && <>
                <div>Mises à jour trouvées : {updateResult.updateCount ?? 0}{updateResult.truncated ? ' (liste partielle)' : ''}</div>
                <div>Redémarrage requis : {updateResult.rebootRequired == null ? 'Inconnu' : updateResult.rebootRequired ? 'Oui' : 'Non'}</div>
                {updateResult.checkedAtUtc && <div>Dernière recherche : {new Date(updateResult.checkedAtUtc).toLocaleString('fr-FR')}</div>}
              </>}
              {updateResult.source && <div>Source : {updateResult.source}</div>}
              {updateResult.note && <div style={{ marginTop: 5 }}>{updateResult.note}</div>}
            </div>}
          </div>
          {updateResult?.updates && updateResult.updates.length > 0 && <Group title="Mises à jour proposées">
            {updateResult.updates.map((item, index) => <Row
              key={`${item.title}-${index}`}
              title={item.title}
              description={[
                item.kbArticleIds?.length ? `KB${item.kbArticleIds.join(', KB')}` : '',
                item.isMandatory ? 'Obligatoire' : '',
                item.isDownloaded ? 'Déjà téléchargée' : '',
              ].filter(Boolean).join(' · ') || 'Mise à jour logicielle proposée par Windows Update'}
              last={index === updateResult.updates!.length - 1}
            />)}
          </Group>}
          <button type="button" style={{ ...button, opacity: checkingUpdates ? 0.65 : 1 }} disabled={checkingUpdates} onClick={() => void checkWindowsUpdates()}>
            {checkingUpdates ? 'Recherche en cours…' : 'Rechercher les mises à jour Windows'}
          </button>
          <Group title="Préférences SYSTEMES">
            <Row title="Mises à jour automatiques" description="Préférence locale de SYSTEMES ; elle ne modifie pas la stratégie Windows Update." control={toggle(s.autoUpdate, () => s.setAutoUpdate(!s.autoUpdate), 'Mises à jour automatiques')} last />
          </Group>
        </>
      case 'about':
        return <>
          <div style={{ ...panel, padding: 22, display: 'flex', alignItems: 'center', gap: 16 }}><FluentIcon name="monitor" size={42} color="var(--win-accent)" /><div><div style={{ fontSize: 20, fontWeight: 650 }}>SYSTEMES</div><div style={muted}>Environnement de bureau inspiré de Windows 11</div></div></div>
          <Group title="Informations sur l’environnement">
            <Row title="Nom" description="SYSTEMES" />
            <Row title="Version" description="0.1.0" />
            <Row title="Plateforme" description={typeof navigator !== 'undefined' ? navigator.platform || 'Navigateur web' : 'Non disponible'} />
            <Row title="Navigateur" description={typeof navigator !== 'undefined' ? navigator.userAgent : 'Non disponible'} last />
          </Group>
        </>
      default: return null
    }
  }

  return <div style={shell}>
    <aside style={sidebar}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '2px 8px 8px' }}><FluentIcon name="settings" size={22} color="var(--win-accent)" /><span style={{ fontSize: 17, fontWeight: 650 }}>Paramètres</span></div>
      <div style={{ position: 'relative', padding: '0 4px 6px' }}><FluentIcon name="search" size={15} color="var(--text-secondary)" /><input aria-label="Rechercher un paramètre" value={query} onChange={e => setQuery(e.target.value)} placeholder="Rechercher un paramètre" style={{ width: '100%', boxSizing: 'border-box', border: '1px solid var(--window-border)', borderRadius: 6, padding: '10px 10px 10px 30px', background: 'var(--window-bg)', color: 'var(--text-primary)', fontSize: 12 }} /></div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: 12, borderRadius: 8, background: 'var(--window-bg)', border: '1px solid var(--window-border)' }}><div style={{ width: 38, height: 38, borderRadius: '50%', background: 'var(--win-accent)', color: '#fff', display: 'grid', placeItems: 'center', fontWeight: 650 }}>U</div><div><div style={{ fontSize: 13, fontWeight: 650 }}>Utilisateur</div><div style={muted}>Compte local</div></div></div>
      <nav aria-label="Catégories des paramètres" style={{ display: 'flex', flexDirection: 'column', gap: 3, overflowY: 'auto' }}>
        {filtered.map(item => <button key={item.id} type="button" onClick={() => go(item.id)} aria-current={section === item.id ? 'page' : undefined} style={{ display: 'flex', alignItems: 'center', gap: 12, textAlign: 'left', width: '100%', border: 0, borderRadius: 6, padding: '10px 12px', cursor: 'pointer', fontSize: 12.5, color: 'var(--text-primary)', background: section === item.id ? 'var(--win-accent-light)' : 'transparent', borderLeft: section === item.id ? '3px solid var(--win-accent)' : '3px solid transparent' }}><FluentIcon name={item.icon} size={18} color={section === item.id ? 'var(--win-accent)' : 'var(--text-secondary)'} /><span>{item.label}</span></button>)}
        {filtered.length === 0 && <p style={{ ...muted, padding: 12 }}>Aucune catégorie correspondante.</p>}
      </nav>
    </aside>
    <main style={content}><div style={{ maxWidth: 900, margin: '0 auto' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, color: 'var(--text-secondary)', fontSize: 11, marginBottom: 12 }}>Paramètres <FluentIcon name="chevron-right" size={12} /> {current.label}</div>
      <h1 style={{ fontSize: 30, fontWeight: 650, letterSpacing: '-.7px', margin: '0 0 6px' }}>{current.label}</h1>
      <p style={{ ...muted, margin: '0 0 24px' }}>{current.description}</p>
      {renderPage()}
    </div></main>
  </div>
}
