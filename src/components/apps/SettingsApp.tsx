import React, { useEffect, useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { WALLPAPERS } from '../../data/constants'
import { NativeBridge } from '../../services/NativeBridge'

type SettingsSection =
  | 'home'
  | 'system'
  | 'bluetooth'
  | 'network'
  | 'personalization'
  | 'apps'
  | 'accounts'
  | 'time'
  | 'gaming'
  | 'accessibility'
  | 'privacy'
  | 'update'
  | 'about'

const ACCENT_COLORS = [
  { color: '#0078d4', name: 'Bleu Windows' },
  { color: '#00b4d8', name: 'Cyan' },
  { color: '#008272', name: 'Sarcelle' },
  { color: '#107c41', name: 'Vert forêt' },
  { color: '#8764b8', name: 'Iris' },
  { color: '#744da9', name: 'Violet' },
  { color: '#8b5cf6', name: 'Neon Violet' },
  { color: '#d13438', name: 'Cramoisi' },
  { color: '#ca5010', name: 'Orange' },
  { color: '#e3008c', name: 'Rose vif' },
  { color: '#ff4b82', name: 'Rose' },
  { color: '#ffb900', name: 'Or' },
]

// Re-usable toggle switch component
const Toggle: React.FC<{ value: boolean; onChange: () => void }> = ({ value, onChange }) => (
  <button
    onClick={onChange}
    style={{
      width: '48px',
      height: '26px',
      borderRadius: '14px',
      border: 'none',
      backgroundColor: value ? 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))' : 'rgba(255,255,255,0.08)',
      cursor: 'pointer',
      position: 'relative',
      transition: 'background 0.25s ease, box-shadow 0.25s ease',
      flexShrink: 0,
      boxShadow: value ? '0 0 12px rgba(139,92,246,0.35), inset 0 0 6px rgba(255,255,255,0.15)' : 'none',
    }}
  >
    <span
      style={{
        position: 'absolute',
        top: '3px',
        left: value ? '25px' : '3px',
        width: '20px',
        height: '20px',
        borderRadius: '50%',
        background: 'linear-gradient(145deg, #fff, #e0e0e0)',
        transition: 'left 0.25s cubic-bezier(0.4, 0, 0.2, 1)',
        boxShadow: '0 2px 6px rgba(0,0,0,0.35), 0 0 4px rgba(0,0,0,0.2)',
      }}
    />
  </button>
)

// Row with a toggle
const SettingRow: React.FC<{
  icon?: string
  title: string
  desc?: string
  toggle?: boolean
  onToggle?: () => void
  onClick?: () => void
  badge?: string
  children?: React.ReactNode
}> = ({ icon, title, desc, toggle, onToggle, onClick, badge, children }) => (
  <div
    onClick={onClick}
    style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '16px 20px',
      borderRadius: '14px',
      backgroundColor: 'rgba(139,92,246,0.06)',
      border: '1px solid rgba(139,92,246,0.2)',
      marginBottom: '10px',
      cursor: onClick ? 'pointer' : 'default',
      transition: 'all 0.18s ease',
      gap: '14px',
      boxShadow: '0 2px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)',
    }}
    onMouseEnter={e => { if (onClick) { e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.14)'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.45)'; e.currentTarget.style.boxShadow = '0 4px 20px rgba(139,92,246,0.15), inset 0 1px 0 rgba(255,255,255,0.08)'; } }}
    onMouseLeave={e => { if (onClick) { e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.06)'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.2)'; e.currentTarget.style.boxShadow = '0 2px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)'; } }}
  >
    <div style={{ display: 'flex', alignItems: 'center', gap: '16px', flex: 1, minWidth: 0 }}>
      {icon && (
        <div style={{
          width: '38px', height: '38px', borderRadius: '10px',
          background: 'linear-gradient(135deg, rgba(139,92,246,0.2), rgba(0,212,255,0.1))',
          border: '1px solid rgba(139,92,246,0.25)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          flexShrink: 0,
        }}>
          <FluentIcon name={icon} size={18} color="var(--win-accent)" />
        </div>
      )}
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)', letterSpacing: '0.01em' }}>{title}</div>
        {desc && <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)', marginTop: '3px', lineHeight: 1.4 }}>{desc}</div>}
        {children}
      </div>
    </div>
    <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexShrink: 0 }}>
      {badge && (
        <span style={{
          fontSize: '10.5px', padding: '3px 10px', borderRadius: '12px',
          backgroundColor: 'rgba(139,92,246,0.15)', color: 'var(--win-accent)', fontWeight: 700,
          border: '1px solid rgba(139,92,246,0.2)', letterSpacing: '0.02em',
        }}>{badge}</span>
      )}
      {toggle !== undefined && onToggle && <Toggle value={toggle} onChange={onToggle} />}
      {onClick && !toggle && <FluentIcon name="chevron-right" size={16} color="var(--text-tertiary)" />}
    </div>
  </div>
)

const SectionTitle: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <h3 style={{
    fontSize: '11px',
    fontWeight: 700,
    color: 'var(--win-accent)',
    textTransform: 'uppercase',
    letterSpacing: '0.12em',
    marginBottom: '10px',
    marginTop: '24px',
    textShadow: '0 0 10px rgba(139,92,246,0.35)',
  }}>
    {children}
  </h3>
)

export const SettingsApp: React.FC = () => {
  const {
    theme, setTheme,
    wallpaper, setWallpaper,
    accentColor, setAccentColor,
    brightness, setBrightness,
    volume, setVolume,
    isMuted, toggleMute,
    wifiEnabled,
    nightLight, toggleNightLight,
    batterySaver, toggleBatterySaver,
    fontSize, setFontSize,
    resolution, setResolution,
    refreshRate, setRefreshRate,
    focusAssist, setFocusAssist,
    taskbarAlignment, setTaskbarAlignment,
    cameraEnabled, setCameraEnabled,
    micEnabled, setMicEnabled,
    diagnosticData, setDiagnosticData,
    autoUpdate, setAutoUpdate,
    notificationsEnabled, toggleNotifications,
    addNotification,
    airplaneMode, mobileHotspot,
    toggleAirplaneMode, toggleMobileHotspot,
    gameBarEnabled, toggleGameBar,
    gameModeEnabled, toggleGameMode,
    audioRecordingEnabled, toggleAudioRecording,
    vrrEnabled, toggleVRR,
    hardwareGpuScheduling, toggleHardwareGpuScheduling,
    magnifierEnabled, toggleMagnifier,
    colorFilterEnabled, toggleColorFilter,
    highContrastEnabled, toggleHighContrast,
    captionsEnabled, toggleCaptions,
    audioAlertsEnabled, toggleAudioAlerts,
    voiceAccessEnabled, toggleVoiceAccess,
    personalizedExperiences, togglePersonalizedExperiences,
    activityHistory, toggleActivityHistory,
    syncSettings, toggleSyncSettings,
    setTimeAutomatically, toggleSetTimeAutomatically,
    setTimeZoneAutomatically, toggleSetTimeZoneAutomatically,
    lockScreenBackground, toggleLockScreenBackground,
    lockScreenAppStatus, toggleLockScreenAppStatus,
  } = useOS()

  const [section, setSection] = useState<SettingsSection>('home')
  const [searchQuery, setSearchQuery] = useState('')
  const [hardwareInfo, setHardwareInfo] = useState<any>(null)
  const [deviceInfo, setDeviceInfo] = useState<{ usb?: string[]; network?: string[] } | null>(null)
  const [powerSchemes, setPowerSchemes] = useState<{ guid: string; name: string }[]>([])
  const [activePowerGuid, setActivePowerGuid] = useState('')
  const [selectedPowerGuid, setSelectedPowerGuid] = useState('')
  const [powerMessage, setPowerMessage] = useState('')
  const [services, setServices] = useState<{ name: string; displayName: string; state: string; startMode: string; processId: number }[]>([])
  const [serviceMessage, setServiceMessage] = useState('')
  const [busyService, setBusyService] = useState<string | null>(null)
  const [updateMessage, setUpdateMessage] = useState('')
  const [wifiStatus, setWifiStatus] = useState<{ connected: boolean; ssid?: string | null; state?: string | null; signal?: string | null } | null>(null)
  const [wifiNetworks, setWifiNetworks] = useState<{ ssid: string; signal?: string; authentication?: string; encryption?: string }[]>([])
  const [wifiMessage, setWifiMessage] = useState('')
  const [wifiBusy, setWifiBusy] = useState(false)
  const [bluetoothStatus, setBluetoothStatus] = useState<{ adapterPresent: boolean; powered: boolean; adapterName?: string | null; devices?: { id: string; name: string; status: string }[] } | null>(null)
  const [nearbyBluetooth, setNearbyBluetooth] = useState<{ id: string; name: string }[]>([])
  const [bluetoothMessage, setBluetoothMessage] = useState('')
  const [bluetoothBusy, setBluetoothBusy] = useState(false)

  const refreshWifi = async () => {
    setWifiBusy(true)
    setWifiMessage('')
    try {
      const [status, scan] = await Promise.all([NativeBridge.readWifiStatus(), NativeBridge.scanWifi()])
      setWifiStatus(status)
      setWifiNetworks(Array.isArray(scan?.networks) ? scan.networks : [])
      if (scan?.error) setWifiMessage(scan.error)
    } catch (error) {
      setWifiMessage(error instanceof Error ? error.message : String(error))
    } finally { setWifiBusy(false) }
  }

  const refreshBluetooth = async () => {
    setBluetoothBusy(true)
    setBluetoothMessage('')
    try {
      const result = await NativeBridge.scanBluetooth(6)
      setNearbyBluetooth(Array.isArray(result?.devices) ? result.devices : [])
      if (result?.error) setBluetoothMessage(result.error)
    } catch (error) { setBluetoothMessage(error instanceof Error ? error.message : String(error)) }
    finally { setBluetoothBusy(false) }
  }

  const pairBluetooth = async (device: { id: string; name: string }) => {
    if (!window.confirm('Demander à Windows de coupler « ' + device.name + ' » ?')) return
    try {
      const result: any = await NativeBridge.pairBluetoothDevice(device.id)
      setBluetoothMessage(result?.paired ? 'Appairage confirmé par Windows.' : 'Windows n’a pas confirmé l’appairage : ' + (result?.status || 'inconnu'))
      const status = await NativeBridge.readBluetoothStatus()
      setBluetoothStatus(status)
    } catch (error) { setBluetoothMessage(error instanceof Error ? error.message : String(error)) }
  }

  const connectToWifi = async (ssid: string, connected: boolean) => {
    if (connected) {
      if (!window.confirm('Déconnecter le Wi-Fi actuel ?')) return
      try { await NativeBridge.disconnectWifi(); await refreshWifi() }
      catch (error) { setWifiMessage(error instanceof Error ? error.message : String(error)) }
      return
    }
    const password = window.prompt('Mot de passe Wi-Fi (laisser vide si le réseau est ouvert)') || ''
    try { await NativeBridge.connectWifi(ssid, password); await refreshWifi() }
    catch (error) { setWifiMessage(error instanceof Error ? error.message : String(error)) }
  }

  useEffect(() => {
    if (section === 'about') void NativeBridge.readHardware().then(setHardwareInfo).catch((error: unknown) => setUpdateMessage(error instanceof Error ? error.message : String(error)))
    if (section === 'bluetooth' || section === 'network') void NativeBridge.readDevices().then((value: any) => setDeviceInfo(value)).catch((error: unknown) => setUpdateMessage(error instanceof Error ? error.message : String(error)))
    if (section === 'bluetooth') void NativeBridge.readBluetoothStatus().then(setBluetoothStatus).catch((error: unknown) => setBluetoothMessage(error instanceof Error ? error.message : String(error)))
    if (section === 'network') void refreshWifi()
    if (section === 'system') void NativeBridge.readPowerSchemes().then((value: any) => {
      const schemes = Array.isArray(value?.schemes) ? value.schemes : []
      setPowerSchemes(schemes)
      setActivePowerGuid(value?.activeGuid || '')
      setSelectedPowerGuid(value?.activeGuid || '')
    }).catch((error: unknown) => setPowerMessage(error instanceof Error ? error.message : String(error)))
    if (section === 'system') void NativeBridge.readServices().then((value: any) => setServices(Array.isArray(value?.services) ? value.services : [])).catch((error: unknown) => setServiceMessage(error instanceof Error ? error.message : String(error)))
  }, [section])

  const applyPowerScheme = async () => {
    if (!selectedPowerGuid || !window.confirm('Appliquer ce plan d’alimentation Windows ?')) return
    try {
      const result: any = await NativeBridge.setPowerScheme(selectedPowerGuid)
      const snapshot: any = await NativeBridge.readPowerSchemes()
      setActivePowerGuid(snapshot?.activeGuid || result?.activeGuid || '')
      setPowerMessage('Plan actif : ' + (result?.activeName || 'confirmé par Windows'))
    } catch (error) { setPowerMessage(error instanceof Error ? error.message : String(error)) }
  }

  const verifyWindowsUpdateService = async () => {
    try {
      const result: any = await NativeBridge.checkWindowsUpdate()
      setUpdateMessage(result?.serviceRunning ? 'Service Windows Update actif' : 'Service Windows Update arrêté')
    } catch (error) { setUpdateMessage(error instanceof Error ? error.message : String(error)) }
  }

  const changeServiceState = async (service: { name: string; displayName: string; state: string }) => {
    const action = service.state.toLowerCase() === 'running' ? 'stop' : 'start'
    if (!window.confirm((action === 'stop' ? 'Arrêter' : 'Démarrer') + ' le service Windows « ' + service.displayName + ' » ?')) return
    setBusyService(service.name)
    setServiceMessage('')
    try {
      await NativeBridge.controlService(service.name, action)
      const result: any = await NativeBridge.readServices()
      setServices(Array.isArray(result?.services) ? result.services : [])
      setServiceMessage('État de « ' + service.displayName + ' » confirmé par Windows.')
    } catch (error) {
      setServiceMessage(error instanceof Error ? error.message : String(error))
    } finally { setBusyService(null) }
  }

  const navItems: { id: SettingsSection; label: string; icon: string; desc?: string }[] = [
    { id: 'home', label: 'Accueil', icon: 'pc' },
    { id: 'system', label: 'Système', icon: 'sliders', desc: 'Écran, son, notifications, alimentation' },
    { id: 'bluetooth', label: 'Bluetooth et appareils', icon: 'battery', desc: 'Souris, clavier, casque, autres appareils' },
    { id: 'network', label: 'Réseau et Internet', icon: 'wifi', desc: 'Wi-Fi, réseau câblé, VPN' },
    { id: 'personalization', label: 'Personnalisation', icon: 'palette', desc: 'Arrière-plan, couleurs, écran de verrouillage' },
    { id: 'apps', label: 'Applications', icon: 'square', desc: 'Applications installées, fonctionnalités optionnelles' },
    { id: 'accounts', label: 'Comptes', icon: 'lock', desc: 'Compte Microsoft, famille, autres utilisateurs' },
    { id: 'time', label: 'Heure et langue', icon: 'clock', desc: 'Fuseau horaire, région, langue d\'affichage' },
    { id: 'gaming', label: 'Jeux', icon: 'terminal', desc: 'Xbox Game Bar, Game Mode, captures' },
    { id: 'accessibility', label: 'Accessibilité', icon: 'minimize', desc: 'Loupe, narrateur, contraste élevé' },
    { id: 'privacy', label: 'Confidentialité et sécurité', icon: 'lock', desc: 'Autorisations Windows, données de diagnostic' },
    { id: 'update', label: 'Windows Update', icon: 'refresh', desc: 'Mises à jour, Windows Insider' },
    { id: 'about', label: 'À propos', icon: 'file-text', desc: 'Spécifications de l\'appareil et Windows' },
  ]

  const filteredNav = searchQuery.trim()
    ? navItems.filter(n => n.label.toLowerCase().includes(searchQuery.toLowerCase()) || n.desc?.toLowerCase().includes(searchQuery.toLowerCase()))
    : navItems

  return (
    <div style={{ display: 'flex', height: '100%', backgroundColor: 'var(--window-bg)', fontFamily: 'var(--font-segoe)', overflow: 'hidden' }}>

      {/* ──── Left Sidebar ──── */}
      <div style={{
        width: '270px',
        borderRight: '1px solid rgba(139,92,246,0.2)',
        display: 'flex',
        flexDirection: 'column',
        background: 'linear-gradient(180deg, rgba(10,10,18,0.95), rgba(10,10,18,0.98))',
        flexShrink: 0,
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
      }}>
        {/* User profile */}
        <div style={{ padding: '24px 16px 14px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', padding: '14px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(139,92,246,0.1), rgba(0,212,255,0.06))', border: '1px solid rgba(139,92,246,0.2)', marginBottom: '14px', boxShadow: '0 4px 20px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.04)' }}>
            <div style={{ width: '46px', height: '46px', borderRadius: '50%', background: 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))', display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#fff', fontWeight: 700, fontSize: '18px', flexShrink: 0, boxShadow: '0 0 16px rgba(139,92,246,0.5)' }}>W</div>
            <div>
              <div style={{ fontSize: '14px', fontWeight: 700, color: 'var(--text-primary)' }}>Utilisateur Windows</div>
              <div style={{ fontSize: '11.5px', color: 'var(--win-accent-cyan)', marginTop: '3px', fontWeight: 500 }}>eemntirey@outlook.com</div>
            </div>
          </div>

          {/* Search */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', padding: '10px 14px', backgroundColor: 'rgba(255,255,255,0.04)', borderRadius: '10px', border: '1px solid rgba(139,92,246,0.15)', boxShadow: 'inset 0 2px 6px rgba(0,0,0,0.2)' }}>
            <FluentIcon name="search" size={15} color="var(--win-accent)" />
            <input
              type="text"
              placeholder="Rechercher dans les paramètres"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              style={{ border: 'none', background: 'transparent', outline: 'none', fontSize: '12.5px', color: 'var(--text-primary)', width: '100%', fontFamily: 'inherit' }}
            />
          </div>
        </div>

        {/* Nav */}
        <div style={{ flex: 1, overflowY: 'auto', padding: '0 8px 8px' }}>
          {filteredNav.map(item => {
            const active = section === item.id
            return (
              <button
                key={item.id}
                onClick={() => setSection(item.id)}
                style={{
                  width: '100%', display: 'flex', alignItems: 'center', gap: '12px',
                  padding: '10px 14px', borderRadius: '10px', border: 'none',
                  background: active ? 'linear-gradient(135deg, rgba(139,92,246,0.2), rgba(0,212,255,0.1))' : 'transparent',
                  color: active ? 'var(--win-accent)' : 'var(--text-primary)',
                  fontSize: '13px', fontWeight: active ? 600 : 400, cursor: 'pointer',
                  textAlign: 'left', transition: 'all 0.15s ease', marginBottom: '2px',
                  borderLeft: active ? '3px solid var(--win-accent)' : '3px solid transparent',
                  boxShadow: active ? 'inset 0 1px 0 rgba(255,255,255,0.08), 0 2px 8px rgba(139,92,246,0.15)' : 'none',
                }}
                onMouseEnter={e => { if (!active) { e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.08)'; } }}
                onMouseLeave={e => { if (!active) { e.currentTarget.style.backgroundColor = 'transparent'; } }}
              >
                <FluentIcon name={item.icon} size={18} color={active ? 'var(--win-accent-cyan)' : 'var(--text-secondary)'} />
                <span>{item.label}</span>
              </button>
            )
          })}
        </div>
      </div>

      {/* ──── Main Content ──── */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '28px 36px', background: 'linear-gradient(180deg, rgba(139,92,246,0.03) 0%, rgba(10,10,18,0) 60%)' }}>

        {/* HOME */}
        {section === 'home' && (
          <div>
            <h1 style={{ fontSize: '36px', fontWeight: 800, marginBottom: '6px', letterSpacing: '-0.03em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Paramètres</h1>
            <p style={{ fontSize: '13.5px', color: 'var(--win-accent-cyan)', marginBottom: '30px', marginTop: '6px', fontWeight: 500 }}>eemntirey@outlook.com</p>

            {/* Recommended settings cards */}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px', marginBottom: '28px' }}>
              {[
                { icon: 'moon', title: 'Mode Sombre activé', desc: 'Personnalisation › Couleurs', action: () => setSection('personalization') },
                { icon: 'wifi', title: wifiEnabled ? 'Wi-Fi connecté' : 'Wi-Fi désactivé', desc: 'Réseau et Internet', action: () => setSection('network') },
                { icon: 'refresh', title: 'Windows à jour', desc: 'Dernière vérif : aujourd\'hui', action: () => setSection('update') },
                { icon: 'battery-charging', title: 'Batterie 94%', desc: 'Alimentation et veille', action: () => setSection('system') },
              ].map(card => (
                <div key={card.title} onClick={card.action} style={{
                  padding: '20px 22px', borderRadius: '18px',
                  background: 'linear-gradient(135deg, rgba(139,92,246,0.08), rgba(0,212,255,0.06))',
                  border: '1px solid rgba(139,92,246,0.2)',
                  cursor: 'pointer', transition: 'all 0.2s ease',
                  boxShadow: '0 8px 28px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)',
                }}
                  onMouseEnter={e => {
                    e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.14), rgba(0,212,255,0.1))';
                    e.currentTarget.style.borderColor = 'rgba(139,92,246,0.45)';
                    e.currentTarget.style.transform = 'translateY(-3px)';
                    e.currentTarget.style.boxShadow = '0 14px 35px rgba(139,92,246,0.2), inset 0 1px 0 rgba(255,255,255,0.08)';
                  }}
                  onMouseLeave={e => {
                    e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.08), rgba(0,212,255,0.06))';
                    e.currentTarget.style.borderColor = 'rgba(139,92,246,0.2)';
                    e.currentTarget.style.transform = 'none';
                    e.currentTarget.style.boxShadow = '0 8px 28px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)';
                  }}
                >
                  <div style={{
                    width: '44px', height: '44px', borderRadius: '14px',
                    background: 'linear-gradient(135deg, rgba(139,92,246,0.25), rgba(0,212,255,0.2))',
                    border: '1px solid rgba(139,92,246,0.3)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    boxShadow: '0 0 14px rgba(139,92,246,0.25)',
                  }}>
                    <FluentIcon name={card.icon} size={22} color="#fff" />
                  </div>
                  <div style={{ fontSize: '14px', fontWeight: 700, marginTop: '14px', color: 'var(--text-primary)' }}>{card.title}</div>
                  <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)', marginTop: '5px', lineHeight: 1.4 }}>{card.desc}</div>
                </div>
              ))}
            </div>

            <h2 style={{ fontSize: '18px', fontWeight: 700, marginBottom: '14px', color: 'var(--text-primary)', letterSpacing: '0.01em' }}>Toutes les catégories</h2>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '12px' }}>
              {navItems.filter(n => n.id !== 'home').map(item => (
                <div key={item.id} onClick={() => setSection(item.id)} style={{ padding: '18px 16px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(139,92,246,0.06), rgba(0,212,255,0.04))', border: '1px solid rgba(139,92,246,0.15)', cursor: 'pointer', transition: 'all 0.2s ease', boxShadow: '0 4px 16px rgba(0,0,0,0.2)' }}
                  onMouseEnter={e => { e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.12), rgba(0,212,255,0.08))'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.35)'; e.currentTarget.style.transform = 'translateY(-2px)'; e.currentTarget.style.boxShadow = '0 10px 28px rgba(139,92,246,0.15)'; }}
                  onMouseLeave={e => { e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.06), rgba(0,212,255,0.04))'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.15)'; e.currentTarget.style.transform = 'none'; e.currentTarget.style.boxShadow = '0 4px 16px rgba(0,0,0,0.2)'; }}
                >
                  <div style={{
                    width: '40px', height: '40px', borderRadius: '12px',
                    background: 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    boxShadow: '0 0 14px rgba(139,92,246,0.3)', marginBottom: '10px',
                  }}>
                    <FluentIcon name={item.icon} size={20} color="#fff" />
                  </div>
                  <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)' }}>{item.label}</div>
                  {item.desc && <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)', marginTop: '4px', lineHeight: 1.35 }}>{item.desc}</div>}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* SYSTEM */}
        {section === 'system' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Système</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Écran, son, notifications, alimentation</p>

            <SectionTitle>Plan d’alimentation Windows</SectionTitle>
            <div style={{ display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap', marginBottom: 8 }}>
              <select aria-label="Plan d’alimentation Windows" value={selectedPowerGuid} onChange={event => setSelectedPowerGuid(event.target.value)} style={{ minWidth: 220, padding: 9, background: 'var(--flyout-card)', color: 'var(--text-primary)', border: '1px solid var(--window-border)', borderRadius: 8 }}>
                {powerSchemes.map(item => <option key={item.guid} value={item.guid}>{item.name}</option>)}
              </select>
              <button type="button" aria-label="Appliquer le plan à Windows" onClick={() => void applyPowerScheme()} disabled={!selectedPowerGuid || selectedPowerGuid === activePowerGuid} style={{ padding: '9px 12px', borderRadius: 8, border: 0, background: 'var(--win-accent)', color: '#fff' }}>Appliquer le plan à Windows</button>
            </div>
            {powerMessage && <div role="status" style={{ fontSize: 12, marginBottom: 12 }}>{powerMessage}</div>}

            <SectionTitle>Services Windows</SectionTitle>
            <div style={{ maxHeight: 230, overflow: 'auto', border: '1px solid var(--window-border)', borderRadius: 8, marginBottom: 8 }}>
              {services.map(service => <div key={service.name} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, padding: '8px 10px', borderBottom: '1px solid var(--window-border)', fontSize: 11 }}>
                <div style={{ minWidth: 0 }}><div style={{ fontWeight: 600 }}>{service.displayName || service.name}</div><div style={{ color: 'var(--text-tertiary)' }}>{service.name} · {service.state} · {service.startMode}</div></div>
                <button type="button" disabled={busyService === service.name || service.startMode === 'Disabled'} onClick={() => void changeServiceState(service)} style={{ flexShrink: 0, padding: '6px 9px', borderRadius: 6 }}>{busyService === service.name ? '…' : service.state.toLowerCase() === 'running' ? 'Arrêter' : 'Démarrer'}</button>
              </div>)}
              {services.length === 0 && <div style={{ padding: 12, fontSize: 11, color: 'var(--text-tertiary)' }}>Aucun service chargé ou accès WMI indisponible.</div>}
            </div>
            {serviceMessage && <div role="status" style={{ fontSize: 11, marginBottom: 12 }}>{serviceMessage}</div>}

            <SectionTitle>Affichage</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(139,92,246,0.3), transparent)', borderRadius: '2px', marginBottom: '14px' }}></div>
            <SettingRow icon="sun" title="Luminosité" desc={`${brightness}% — Ajustez la luminosité intégrée`}>
              <input type="range" min="20" max="100" value={brightness} onChange={e => setBrightness(Number(e.target.value))} style={{ width: '220px', accentColor: 'var(--win-accent-cyan)', marginTop: '8px', cursor: 'pointer', height: '6px', borderRadius: '3px', background: 'rgba(255,255,255,0.1)' }} />
            </SettingRow>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '10px', marginBottom: '10px' }}>
              <div style={{ padding: '16px 18px', borderRadius: '12px', background: 'linear-gradient(135deg, rgba(139,92,246,0.06), rgba(0,212,255,0.04))', border: '1px solid rgba(139,92,246,0.2)', boxShadow: '0 4px 16px rgba(0,0,0,0.2)' }}>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '8px', fontWeight: 600, letterSpacing: '0.05em', textTransform: 'uppercase' }}>Résolution d'affichage</div>
                <select value={resolution} onChange={e => setResolution(e.target.value)} style={{ background: 'rgba(0,0,0,0.3)', border: '1px solid rgba(139,92,246,0.2)', borderRadius: '8px', color: 'var(--text-primary)', fontSize: '13px', fontWeight: 600, cursor: 'pointer', width: '100%', outline: 'none', padding: '8px 10px', appearance: 'none' }}>
                  {['2560x1440', '1920x1080', '1680x1050', '1280x720'].map(r => <option key={r} value={r} style={{ background: '#1a1a2e', color: '#fff' }}>{r}</option>)}
                </select>
              </div>
              <div style={{ padding: '16px 18px', borderRadius: '12px', background: 'linear-gradient(135deg, rgba(139,92,246,0.06), rgba(0,212,255,0.04))', border: '1px solid rgba(139,92,246,0.2)', boxShadow: '0 4px 16px rgba(0,0,0,0.2)' }}>
                <div style={{ fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '8px', fontWeight: 600, letterSpacing: '0.05em', textTransform: 'uppercase' }}>Fréquence de rafraîchissement</div>
                <select value={refreshRate} onChange={e => setRefreshRate(e.target.value)} style={{ background: 'rgba(0,0,0,0.3)', border: '1px solid rgba(139,92,246,0.2)', borderRadius: '8px', color: 'var(--text-primary)', fontSize: '13px', fontWeight: 600, cursor: 'pointer', width: '100%', outline: 'none', padding: '8px 10px', appearance: 'none' }}>
                  {['144Hz', '120Hz', '75Hz', '60Hz'].map(r => <option key={r} value={r} style={{ background: '#1a1a2e', color: '#fff' }}>{r}</option>)}
                </select>
              </div>
            </div>
            <SettingRow icon="moon" title="Éclairage nocturne" desc="Réduit la lumière bleue le soir pour protéger vos yeux" toggle={nightLight} onToggle={toggleNightLight} />
            <SettingRow icon="maximize" title="Taille du texte" desc={`${fontSize}% — Mise à l'échelle de l'affichage`}>
              <input type="range" min="80" max="200" step="5" value={fontSize} onChange={e => setFontSize(Number(e.target.value))} style={{ width: '220px', accentColor: 'var(--win-accent)', marginTop: '8px', cursor: 'pointer', height: '6px', borderRadius: '3px', background: 'rgba(255,255,255,0.1)' }} />
            </SettingRow>

            <SectionTitle>Son</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px' }}></div>
            <SettingRow icon="volume" title="Volume maître" desc={isMuted ? 'Muet' : `${volume}%`} toggle={!isMuted} onToggle={toggleMute}>
              {!isMuted && <input type="range" min="0" max="100" value={volume} onChange={e => setVolume(Number(e.target.value))} style={{ width: '220px', accentColor: 'var(--win-accent-cyan)', marginTop: '8px', cursor: 'pointer', height: '6px', borderRadius: '3px', background: 'rgba(255,255,255,0.1)' }} />}
            </SettingRow>

            <SectionTitle>Notifications</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(139,92,246,0.3), transparent)', borderRadius: '2px', marginBottom: '14px' }}></div>
            <SettingRow icon="bell" title="Notifications" desc="Recevoir des notifications des applications et des autres expéditeurs" toggle={notificationsEnabled} onToggle={toggleNotifications} />
            <SettingRow icon="moon" title="Assistance Focus" desc="Masquer les notifications pendant les activités importantes" toggle={focusAssist} onToggle={() => setFocusAssist(!focusAssist)} />

            <SectionTitle>Alimentation</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px' }}></div>
            <SettingRow icon="battery-charging" title="Économiseur de batterie" desc="Réduit la consommation pour prolonger l'autonomie" toggle={batterySaver} onToggle={toggleBatterySaver} />
            <SettingRow icon="power" title="Mise en veille de l'écran" desc="Après 10 minutes d'inactivité" onClick={() => addNotification('Alimentation', 'Ouvrir les options d\'alimentation avancées', 'Paramètres', 'power')} badge="10 min" />
            <SettingRow icon="power" title="Mode veille" desc="Après 30 minutes d'inactivité" onClick={() => addNotification('Alimentation', 'Ouvrir les options d\'alimentation avancées', 'Paramètres', 'power')} badge="30 min" />
          </div>
        )}

        {/* BLUETOOTH */}
        {section === 'bluetooth' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Bluetooth et appareils</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Gérez vos appareils connectés</p>

            <SettingRow icon="sliders" title="Bluetooth" desc={bluetoothStatus?.adapterPresent ? (bluetoothStatus.powered ? 'Adaptateur actif : ' + (bluetoothStatus.adapterName || 'Bluetooth') : 'Adaptateur détecté mais non actif') : 'Adaptateur Bluetooth non détecté'} />
            <div style={{ display: 'flex', gap: 8, margin: '10px 0', flexWrap: 'wrap' }}>
              <button type="button" onClick={() => void refreshBluetooth()} disabled={bluetoothBusy}>{bluetoothBusy ? 'Recherche…' : 'Rechercher des appareils Bluetooth'}</button>
              <button type="button" onClick={() => void NativeBridge.openWindowsSettings('bluetooth')}>Paramètres Bluetooth Windows</button>
            </div>
            {bluetoothMessage && <div role="status" style={{ padding: 8, fontSize: 12 }}>{bluetoothMessage}</div>}
            <SectionTitle>Appareils Bluetooth connus par Windows</SectionTitle>
            {(bluetoothStatus?.devices || []).map(device => <div key={device.id} style={{ padding: 8, marginBottom: 5, background: 'var(--flyout-card)', borderRadius: 7, fontSize: 12 }}>{device.name} · {device.status}</div>)}
            <SectionTitle>Appareils Bluetooth à proximité</SectionTitle>
            {nearbyBluetooth.map(device => <div key={device.id} style={{ display: 'flex', justifyContent: 'space-between', gap: 8, padding: 8, marginBottom: 5, background: 'var(--flyout-card)', borderRadius: 7, fontSize: 12 }}><span>{device.name}</span><button type="button" onClick={() => void pairBluetooth(device)}>Coupler</button></div>)}
            <SectionTitle>Périphériques détectés par Windows</SectionTitle>
            {(deviceInfo?.usb || []).map(name => <div key={name} style={{ padding: 8, marginBottom: 5, background: 'var(--flyout-card)', borderRadius: 7, fontSize: 12 }}>{name}</div>)}
            {(deviceInfo?.network || []).map(name => <div key={name} style={{ padding: 8, marginBottom: 5, background: 'var(--flyout-card)', borderRadius: 7, fontSize: 12 }}>{name}</div>)}
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>

            <SectionTitle>Autres appareils</SectionTitle>
            <SettingRow icon="sliders" title="Imprimantes et scanners" desc="HP LaserJet Pro — Prête" onClick={() => addNotification('Appareils', 'Imprimantes et scanners', 'Paramètres', 'sliders')} />
            <SettingRow icon="pc" title="Appareils USB" desc="2 appareils USB connectés" onClick={() => addNotification('Appareils', 'Appareils USB', 'Paramètres', 'pc')} />
            <SettingRow icon="pc" title="Paramètres du stylo et de la saisie tactile" desc="Personnaliser le comportement du stylo" onClick={() => addNotification('Appareils', 'Stylo et saisie tactile', 'Paramètres', 'pc')} />
            <SettingRow icon="pc" title="Bluetooth mobile" desc="Partager des fichiers via Bluetooth avec des appareils à proximité" onClick={() => addNotification('Appareils', 'Bluetooth mobile', 'Paramètres', 'pc')} />
          </div>
        )}

        {/* NETWORK */}
        {section === 'network' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Réseau et Internet</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Gérez vos connexions réseau</p>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '16px' }}></div>

            <div style={{ padding: '18px 22px', borderRadius: '12px', background: wifiStatus?.connected ? 'rgba(0,120,212,.14)' : 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '16px' }}>
              <div style={{ fontSize: 15, fontWeight: 600 }}>{wifiStatus?.connected ? 'Connecté à ' + (wifiStatus.ssid || 'un réseau Wi-Fi') : 'Aucune connexion Wi-Fi confirmée'}</div>
              <div style={{ fontSize: 12, color: 'var(--text-secondary)' }}>{wifiStatus?.state || 'État lu depuis netsh wlan show interfaces'}{wifiStatus?.signal ? ' · Signal ' + wifiStatus.signal : ''}</div>
            </div>
            <div style={{ display: 'flex', gap: 8, marginBottom: 12, flexWrap: 'wrap' }}>
              <button type="button" onClick={() => void refreshWifi()} disabled={wifiBusy} aria-label="Actualiser les réseaux Wi-Fi">{wifiBusy ? 'Analyse…' : 'Actualiser les réseaux Wi-Fi'}</button>
              <button type="button" onClick={() => void NativeBridge.openWindowsSettings('network-status')}>Paramètres réseau Windows</button>
            </div>
            {wifiMessage && <div role="alert" style={{ padding: 8, marginBottom: 10, color: '#fecaca', fontSize: 12 }}>{wifiMessage}</div>}
            <SectionTitle>Réseaux Wi-Fi détectés par Windows</SectionTitle>
            {wifiNetworks.map(net => {
              const connected = wifiStatus?.connected === true && wifiStatus.ssid === net.ssid
              return <div key={net.ssid} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 18px', borderRadius: 10, background: connected ? 'var(--win-accent-light)' : 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: 6, gap: 10 }}>
                <div><div style={{ fontSize: 13, fontWeight: 600 }}>{net.ssid}{connected ? ' ✓' : ''}</div><div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{net.authentication || 'Sécurité inconnue'} · {net.signal || 'Signal inconnu'}</div></div>
                <button type="button" onClick={() => void connectToWifi(net.ssid, connected)}>{connected ? 'Déconnecter' : 'Se connecter'}</button>
              </div>
            })}
            {!wifiBusy && wifiNetworks.length === 0 && <div style={{ fontSize: 12, color: 'var(--text-tertiary)', padding: 8 }}>Aucun réseau Wi-Fi détecté par Windows.</div>}
            <SectionTitle>Adaptateurs réseau détectés par Windows</SectionTitle>
            {(deviceInfo?.network || []).map(name => <div key={name} style={{ padding: 8, marginBottom: 5, background: 'var(--flyout-card)', borderRadius: 7, fontSize: 12 }}>{name}</div>)}

                        <SectionTitle>Paramètres avancés</SectionTitle>
            <SettingRow icon="sliders" title="Ethernet" desc="Non connecté" onClick={() => addNotification('Réseau', 'Ethernet', 'Paramètres', 'sliders')} />
            <SettingRow icon="lock" title="VPN" desc="Aucun VPN configuré" onClick={() => addNotification('Réseau', 'VPN', 'Paramètres', 'lock')} />
            <SettingRow icon="sliders" title="Mode avion" desc={airplaneMode ? 'Activé' : 'Désactivé'} toggle={airplaneMode} onToggle={toggleAirplaneMode} />
            <SettingRow icon="sliders" title="Point d'accès mobile" desc={mobileHotspot ? 'Activé — partage de connexion' : 'Désactivé'} toggle={mobileHotspot} onToggle={toggleMobileHotspot} />
            <SettingRow icon="sliders" title="Utilisation des données" desc="6,3 Go utilisés ce mois-ci" onClick={() => addNotification('Réseau', 'Utilisation des données', 'Paramètres', 'sliders')} badge="6,3 Go" />
            <SettingRow icon="sliders" title="Paramètres proxy" desc="Aucun proxy configuré" onClick={() => addNotification('Réseau', 'Proxy', 'Paramètres', 'sliders')} />
          </div>
        )}

        {/* PERSONALIZATION */}
        {section === 'personalization' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Personnalisation</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '22px', marginTop: '4px' }}>Personnalisez l'apparence de Windows</p>

            {/* Wallpaper preview */}
            <div style={{ height: '160px', borderRadius: '14px', backgroundImage: `url(${wallpaper.url})`, backgroundSize: 'cover', backgroundPosition: 'center', border: '1px solid var(--window-border)', marginBottom: '20px', position: 'relative', display: 'flex', alignItems: 'flex-end', padding: '14px' }}>
              <div style={{ backgroundColor: 'rgba(0,0,0,0.6)', backdropFilter: 'blur(8px)', padding: '5px 12px', borderRadius: '12px', color: '#fff', fontSize: '12px' }}>
                {wallpaper.name}
              </div>
            </div>

            <SectionTitle>Arrière-plan</SectionTitle>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(120px, 1fr))', gap: '10px', marginBottom: '20px' }}>
              {WALLPAPERS.map(wp => (
                <div key={wp.id} onClick={() => setWallpaper(wp)} style={{ height: '75px', borderRadius: '8px', backgroundImage: `url(${wp.url})`, backgroundSize: 'cover', backgroundPosition: 'center', cursor: 'pointer', border: wallpaper.id === wp.id ? '3px solid var(--win-accent)' : '2px solid transparent', boxShadow: wallpaper.id === wp.id ? '0 0 14px rgba(139,92,246,0.6)' : 'none', transition: 'all 0.15s', position: 'relative' }}
                  onMouseEnter={e => { (e.currentTarget as HTMLElement).style.transform = 'scale(1.04)' }}
                  onMouseLeave={e => { (e.currentTarget as HTMLElement).style.transform = 'scale(1)' }}
                />
              ))}
            </div>

            <SectionTitle>Mode de couleur</SectionTitle>
            <div style={{ display: 'flex', gap: '12px', marginBottom: '16px' }}>
              {[
                { id: 'dark', label: 'Sombre', icon: 'moon', desc: 'Recommandé' },
                { id: 'light', label: 'Clair', icon: 'sun', desc: 'Haute visibilité' },
              ].map(t => (
                <button key={t.id} onClick={() => setTheme(t.id as 'dark' | 'light')} style={{ flex: 1, padding: '14px', borderRadius: '10px', backgroundColor: theme === t.id ? 'var(--win-accent-light)' : 'var(--flyout-card)', border: theme === t.id ? '2px solid var(--win-accent)' : '1px solid var(--window-border)', color: 'var(--text-primary)', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <FluentIcon name={t.icon} size={20} color={theme === t.id ? 'var(--win-accent)' : 'var(--text-secondary)'} />
                  <div style={{ textAlign: 'left' }}>
                    <div style={{ fontSize: '13px', fontWeight: 600 }}>{t.label}</div>
                    <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{t.desc}</div>
                  </div>
                </button>
              ))}
            </div>

            <SectionTitle>Couleur d'accentuation</SectionTitle>
            <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', marginBottom: '16px' }}>
              {ACCENT_COLORS.map(({ color, name }) => {
                const selected = accentColor.toLowerCase() === color.toLowerCase()
                return (
                  <button key={color} title={name} onClick={() => { setAccentColor(color); document.documentElement.style.setProperty('--win-accent', color) }} style={{ width: '34px', height: '34px', borderRadius: '50%', backgroundColor: color, border: selected ? '3px solid #fff' : '2px solid transparent', boxShadow: selected ? `0 0 10px ${color}aa` : 'none', cursor: 'pointer', transition: 'all 0.15s' }} />
                )
              })}
            </div>

            <SectionTitle>Barre des tâches</SectionTitle>
            <div style={{ padding: '14px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '8px' }}>
              <div style={{ fontSize: '13.5px', fontWeight: 500, marginBottom: '10px' }}>Alignement de la barre des tâches</div>
              <div style={{ display: 'flex', gap: '10px' }}>
                {(['center', 'left'] as const).map(align => (
                  <button key={align} onClick={() => setTaskbarAlignment(align)} style={{ flex: 1, padding: '8px', borderRadius: '8px', backgroundColor: taskbarAlignment === align ? 'var(--win-accent-light)' : 'rgba(255,255,255,0.05)', border: taskbarAlignment === align ? '1px solid var(--win-accent)' : '1px solid var(--window-border)', color: 'var(--text-primary)', fontSize: '12px', fontWeight: taskbarAlignment === align ? 600 : 400, cursor: 'pointer' }}>
                    {align === 'center' ? 'Centré (défaut)' : 'Gauche'}
                  </button>
                ))}
              </div>
            </div>
            <SettingRow icon="square" title="Éléments masqués du coin système" desc="Gérez les icônes dans l'angle de la barre des tâches" onClick={() => addNotification('Barre des tâches', 'La gestion individuelle des icônes du coin système n’est pas encore disponible.', 'Paramètres', 'info')} />
            <SettingRow icon="square" title="Comportement de la barre des tâches" desc="Afficher le badge, la sélection de fenêtres, le bouton afficher le bureau" onClick={() => addNotification('Barre des tâches', 'Les options avancées de comportement ne sont pas encore disponibles.', 'Paramètres', 'info')} />

            <SectionTitle>Écran de verrouillage</SectionTitle>
            <SettingRow icon="lock" title="Arrière-plan de l'écran de verrouillage" desc="Windows à la une" toggle={lockScreenBackground} onToggle={toggleLockScreenBackground} />
            <SettingRow icon="clock" title="Afficher l'état des applications" desc="Afficher des informations rapides sur l'écran de verrouillage" toggle={lockScreenAppStatus} onToggle={toggleLockScreenAppStatus} />
          </div>
        )}

        {/* APPS */}
        {section === 'apps' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Applications</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Applications intégrées à SYSTEMES. Cette version ne gère pas de paquets externes.</p>

            <SectionTitle>Applications installées</SectionTitle>
            {[
              { name: 'Photos', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'image' },
              { name: 'Terminal', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'terminal' },
              { name: 'Bloc-notes', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'notepad' },
              { name: 'Paint', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'paint' },
              { name: 'Calculatrice', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'calculator' },
              { name: 'Paramètres', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'settings' },

            ].map(app => (
              <div key={app.name} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '6px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
                  <FluentIcon name={app.icon} size={24} />
                  <div>
                    <div style={{ fontSize: '13.5px', fontWeight: 500 }}>{app.name}</div>
                    <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)' }}>{app.publisher} · {app.size} · v{app.version}</div>
                  </div>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Application système</span>
                </div>
              </div>
            ))}

            <SectionTitle>Fonctionnalités optionnelles</SectionTitle>
            <SettingRow icon="plus" title="Ajouter une fonctionnalité optionnelle" desc="Internet Explorer, OpenSSH, Media Player hérité..." onClick={() => addNotification('Apps', 'Fonctionnalités optionnelles', 'Paramètres', 'plus')} />
            <SettingRow icon="square" title="Applications par défaut" desc="Choisissez les applications par défaut pour le courrier, la musique, les photos..." onClick={() => addNotification('Apps', 'Applications par défaut', 'Paramètres', 'square')} />
          </div>
        )}

        {/* ACCOUNTS */}
        {section === 'accounts' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Comptes</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Votre profil et paramètres de connexion</p>

            {/* Profile card */}
            <div style={{ padding: '20px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(139,92,246,0.15), rgba(0,212,255,0.08))', border: '1px solid rgba(139,92,246,0.35)', marginBottom: '20px', display: 'flex', alignItems: 'center', gap: '20px' }}>
              <div style={{ width: '72px', height: '72px', borderRadius: '50%', background: 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: '28px', fontWeight: 700, color: '#fff', boxShadow: '0 0 20px rgba(139,92,246,0.5)' }}>W</div>
              <div>
                <div style={{ fontSize: '20px', fontWeight: 700 }}>Utilisateur Windows</div>
                <div style={{ fontSize: '13px', color: 'var(--win-accent-cyan)', marginTop: '3px' }}>eemntirey@outlook.com</div>
                <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginTop: '3px' }}>Compte Microsoft · Administrateur</div>
              </div>
            </div>

            <SectionTitle>Paramètres du compte</SectionTitle>
            <SettingRow icon="lock" title="Options de connexion" desc="Windows Hello, code PIN, clé de sécurité, mot de passe" onClick={() => addNotification('Comptes', 'Options de connexion', 'Paramètres', 'lock')} />
            <SettingRow icon="sliders" title="Synchronisation de vos paramètres" desc="Les paramètres, thèmes et mots de passe sont synchronisés" toggle={syncSettings} onToggle={toggleSyncSettings} />
            <SettingRow icon="pc" title="Accès professionnel ou scolaire" desc="Se connecter à l'organisation, MDM ou Azure AD" onClick={() => addNotification('Comptes', 'Accès professionnel ou scolaire', 'Paramètres', 'pc')} />

            <SectionTitle>Autres utilisateurs</SectionTitle>
            <SettingRow icon="plus" title="Ajouter un autre utilisateur" desc="Créer un compte local ou ajouter un compte Microsoft" onClick={() => addNotification('Comptes', 'Ajouter un utilisateur', 'Paramètres', 'plus')} />
          </div>
        )}

        {/* TIME */}
        {section === 'time' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Heure et langue</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Fuseau horaire, région, langue et clavier</p>

            {/* Current time display */}
            <div style={{ padding: '20px', borderRadius: '14px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '20px', textAlign: 'center' }}>
              <div style={{ fontSize: '48px', fontWeight: 200, letterSpacing: '0.05em', color: 'var(--win-accent-cyan)', fontVariantNumeric: 'tabular-nums' }}>
                {new Date().toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}
              </div>
              <div style={{ fontSize: '14px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                {new Date().toLocaleDateString('fr-FR', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' })}
              </div>
            </div>

            <SectionTitle>Date et heure</SectionTitle>
            <SettingRow icon="clock" title="Définir l'heure automatiquement" desc="Synchronisé via time.windows.com" toggle={setTimeAutomatically} onToggle={toggleSetTimeAutomatically} />
            <SettingRow icon="clock" title="Définir le fuseau horaire automatiquement" desc="Europe/Paris (UTC+2)" toggle={setTimeZoneAutomatically} onToggle={toggleSetTimeZoneAutomatically} />
            <SettingRow icon="clock" title="Afficher les heures supplémentaires dans la barre des tâches" desc="Ajouter des horloges pour d'autres fuseaux horaires" onClick={() => addNotification('Heure', 'Horloges supplémentaires', 'Paramètres', 'clock')} />

            <SectionTitle>Région</SectionTitle>
            <div style={{ padding: '14px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '8px' }}>
              <div style={{ fontSize: '12px', color: 'var(--text-secondary)', marginBottom: '6px' }}>Pays ou région</div>
              <select defaultValue="FR" style={{ background: 'transparent', border: 'none', color: 'var(--text-primary)', fontSize: '13.5px', fontWeight: 500, cursor: 'pointer', outline: 'none' }}>
                <option value="FR" style={{ background: '#1a1a2e' }}>France</option>
                <option value="US" style={{ background: '#1a1a2e' }}>États-Unis</option>
                <option value="DE" style={{ background: '#1a1a2e' }}>Allemagne</option>
                <option value="GB" style={{ background: '#1a1a2e' }}>Royaume-Uni</option>
              </select>
            </div>

            <SectionTitle>Langue</SectionTitle>
            <div style={{ padding: '14px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '8px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <div>
                <div style={{ fontSize: '13.5px', fontWeight: 500 }}>Français (France)</div>
                <div style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Langue d'affichage Windows</div>
              </div>
              <span style={{ fontSize: '11px', padding: '2px 8px', borderRadius: '10px', backgroundColor: 'var(--win-accent-light)', color: 'var(--win-accent)' }}>Défaut</span>
            </div>
            <SettingRow icon="plus" title="Ajouter une langue" desc="Installer des packs de langue supplémentaires" onClick={() => addNotification('Heure', 'Ajouter une langue', 'Paramètres', 'plus')} />
          </div>
        )}

        {/* GAMING */}
        {section === 'gaming' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Jeux</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Optimisations pour les jeux et Xbox</p>

            <SectionTitle>Xbox Game Bar</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="terminal" title="Xbox Game Bar" desc="Superposition pour les captures d'écran, l'enregistrement et plus" toggle={gameBarEnabled} onToggle={toggleGameBar} />
            <SettingRow icon="terminal" title="Raccourci clavier Game Bar" desc="Win + G pour ouvrir la Game Bar" onClick={() => addNotification('Jeux', 'Raccourci Game Bar (Win + G)', 'Paramètres', 'terminal')} badge="Win+G" />

            <SectionTitle>Captures</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="square" title="Enregistrement audio lors des captures" desc="Inclure le son du jeu et le microphone" toggle={audioRecordingEnabled} onToggle={toggleAudioRecording} />
            <SettingRow icon="sliders" title="Qualité vidéo" desc="30 fps, qualité standard" onClick={() => addNotification('Jeux', 'Qualité vidéo', 'Paramètres', 'sliders')} badge="30 fps" />
            <SettingRow icon="sliders" title="Dossier des captures" desc="C:/Utilisateurs/eemntirey/Vidéos/Captures" onClick={() => addNotification('Jeux', 'Dossier des captures', 'Paramètres', 'sliders')} />

            <SectionTitle>Mode jeu</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="terminal" title="Mode Jeu" desc="Optimise le PC pour les jeux en priorisant les ressources GPU/CPU" toggle={gameModeEnabled} onToggle={toggleGameMode} badge="Recommandé" />
            <SettingRow icon="sliders" title="Planification GPU accélérée par le matériel" desc="Améliore les performances et la latence vidéo" toggle={hardwareGpuScheduling} onToggle={toggleHardwareGpuScheduling} />
            <SettingRow icon="sliders" title="Variable Refresh Rate (VRR)" desc="Taux de rafraîchissement variable pour les jeux compatibles" toggle={vrrEnabled} onToggle={toggleVRR} />
          </div>
        )}

        {/* ACCESSIBILITY */}
        {section === 'accessibility' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Accessibilité</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Rendez Windows plus facile à utiliser</p>

            <SectionTitle>Vision</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="maximize" title="Taille du texte" desc="Agrandir le texte sans modifier d'autres éléments" onClick={() => addNotification('Accessibilité', 'Taille du texte', 'Paramètres', 'maximize')} />
            <SettingRow icon="maximize" title="Effets visuels" desc="Animations, transparences, fond d'écran animé" onClick={() => addNotification('Accessibilité', 'Effets visuels', 'Paramètres', 'maximize')} />
            <SettingRow icon="maximize" title="Pointeur de la souris et interaction tactile" desc="Modifier l'apparence du curseur" onClick={() => addNotification('Accessibilité', 'Pointeur et tactile', 'Paramètres', 'maximize')} />
            <SettingRow icon="maximize" title="Loupe" desc="Agrandir une zone de l'écran" toggle={magnifierEnabled} onToggle={toggleMagnifier} />
            <SettingRow icon="maximize" title="Filtre de couleur" desc="Daltonisme — Désactivé" toggle={colorFilterEnabled} onToggle={toggleColorFilter} />
            <SettingRow icon="maximize" title="Contraste élevé" desc="Rend le texte et les applications plus faciles à distinguer" toggle={highContrastEnabled} onToggle={toggleHighContrast} />

            <SectionTitle>Audition</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="volume" title="Légendes" desc="Afficher des sous-titres automatiques pour l'audio" toggle={captionsEnabled} onToggle={toggleCaptions} />
            <SettingRow icon="volume" title="Alertes audio" desc="Utiliser des alertes visuelles pour les sons système" toggle={audioAlertsEnabled} onToggle={toggleAudioAlerts} />

            <SectionTitle>Interaction</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="square" title="Clavier" desc="Clavier visuel, touches rémanentes, filtres de touches" onClick={() => addNotification('Accessibilité', 'Clavier', 'Paramètres', 'square')} />
            <SettingRow icon="sliders" title="Souris" desc="Touches souris, vitesse du pointeur" onClick={() => addNotification('Accessibilité', 'Souris', 'Paramètres', 'sliders')} />
            <SettingRow icon="sliders" title="Commande vocale" desc="Dicter du texte et contrôler votre PC par la voix" toggle={voiceAccessEnabled} onToggle={toggleVoiceAccess} />
          </div>
        )}

        {/* PRIVACY */}
        {section === 'privacy' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Confidentialité et sécurité</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Contrôlez vos données et autorisations</p>

            <SectionTitle>Sécurité Windows</SectionTitle>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>
            <SettingRow icon="lock" title="Sécurité Windows" desc="Protection antivirus, pare-feu, performances" onClick={() => addNotification('Confidentialité', 'Sécurité Windows', 'Paramètres', 'lock')} badge="Protégé ✓" />
            <SettingRow icon="lock" title="Chiffrement BitLocker" desc="Lecteur C: chiffré" onClick={() => addNotification('Confidentialité', 'BitLocker', 'Paramètres', 'lock')} badge="Activé" />

            <SectionTitle>Autorisations Windows</SectionTitle>
            <SettingRow icon="sliders" title="Données de diagnostic" desc="Envoyer des données de diagnostic à Microsoft" toggle={diagnosticData} onToggle={() => setDiagnosticData(!diagnosticData)} />
            <SettingRow icon="sliders" title="Expériences personnalisées" desc="Personnaliser les publicités et les recommandations" toggle={personalizedExperiences} onToggle={togglePersonalizedExperiences} />
            <SettingRow icon="sliders" title="Historique des activités" desc="Microsoft collecte les activités de cet appareil" toggle={activityHistory} onToggle={toggleActivityHistory} />

            <SectionTitle>Autorisations des applications</SectionTitle>
            <SettingRow icon="sliders" title="Position" desc="Ouvrir les paramètres de confidentialité de localisation de Windows" onClick={() => { void NativeBridge.openWindowsSettings('privacy-location') }} />
            <SettingRow icon="sliders" title="Caméra" desc="Certaines applications peuvent accéder à la caméra" toggle={cameraEnabled} onToggle={() => setCameraEnabled(!cameraEnabled)} />
            <SettingRow icon="volume" title="Microphone" desc="Certaines applications peuvent accéder au microphone" toggle={micEnabled} onToggle={() => setMicEnabled(!micEnabled)} />
            <SettingRow icon="bell" title="Notifications" desc="Envoyer des notifications depuis les applications" toggle={notificationsEnabled} onToggle={toggleNotifications} />
            <SettingRow icon="sliders" title="Informations du compte" desc="Nom, image et autres informations du compte" onClick={() => addNotification('Confidentialité', 'Informations du compte', 'Paramètres', 'sliders')} />
            <SettingRow icon="sliders" title="Contacts" desc="Accès aux contacts depuis les applications" onClick={() => addNotification('Confidentialité', 'Contacts', 'Paramètres', 'sliders')} />
            <SettingRow icon="clock" title="Historique des appels" desc="Accès à l'historique des appels depuis les applications" onClick={() => addNotification('Confidentialité', 'Historique des appels', 'Paramètres', 'clock')} />
          </div>
        )}

        {/* UPDATE */}
        {section === 'update' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Windows Update</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Maintenir Windows à jour</p>

            <div style={{ display: 'flex', gap: 8, marginBottom: 12, flexWrap: 'wrap' }}>
              <button type="button" onClick={() => void verifyWindowsUpdateService()} style={{ padding: '9px 12px', borderRadius: 8 }}>Vérifier le service</button>
              <button type="button" onClick={() => void NativeBridge.openWindowsSettings('windows-update')} style={{ padding: '9px 12px', borderRadius: 8 }}>Ouvrir Windows Update</button>
            </div>
            {updateMessage && <div role="status" style={{ padding: 8, marginBottom: 12, fontSize: 12 }}><span>{updateMessage}</span><span>La recherche du catalogue de mises à jour n’est pas effectuée par ce contrôle.</span></div>}
            {/* Status banner */}
            <div style={{ padding: '20px 24px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(16,185,129,0.12), rgba(0,212,255,0.08))', border: '1px solid rgba(16,185,129,0.4)', marginBottom: '24px', display: 'flex', alignItems: 'center', gap: '16px' }}>
              <div style={{ width: '44px', height: '44px', borderRadius: '50%', background: 'rgba(16,185,129,0.2)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <FluentIcon name="check" size={24} color="#10b981" />
              </div>
              <div>
                <div style={{ fontSize: '16px', fontWeight: 600, color: '#10b981' }}>Windows est à jour</div>
                <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)', marginTop: '3px' }}>Dernière vérification : aujourd'hui à {new Date().toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}</div>
              </div>
              <button style={{ marginLeft: 'auto', padding: '10px 20px', borderRadius: '10px', border: '1px solid rgba(16,185,129,0.35)', background: 'linear-gradient(135deg, rgba(16,185,129,0.15), rgba(0,212,255,0.1))', color: '#10b981', fontSize: '13px', fontWeight: 700, cursor: 'pointer', transition: 'all 0.2s ease', boxShadow: '0 4px 16px rgba(16,185,129,0.15)' }}
                onMouseEnter={e => { e.currentTarget.style.background = 'linear-gradient(135deg, rgba(16,185,129,0.25), rgba(0,212,255,0.15))'; e.currentTarget.style.boxShadow = '0 6px 24px rgba(16,185,129,0.3)'; }}
                onMouseLeave={e => { e.currentTarget.style.background = 'linear-gradient(135deg, rgba(16,185,129,0.15), rgba(0,212,255,0.1))'; e.currentTarget.style.boxShadow = '0 4px 16px rgba(16,185,129,0.15)'; }}
              >Rechercher des mises à jour</button>
            </div>

            {/* Update history */}
            <SectionTitle>Historique des mises à jour</SectionTitle>
            {[
              { name: 'Mise à jour cumulative Windows 11 (KB5044384)', date: '29 oct. 2026', type: 'Qualité', size: '486 Mo' },
              { name: 'Mise à jour de définitions Windows Defender', date: '31 oct. 2026', type: 'Sécurité', size: '42 Mo' },
              { name: 'Mise à jour du pilote Intel® Wi-Fi 6E AX210', date: '28 oct. 2026', type: 'Pilote', size: '12 Mo' },
            ].map(upd => (
              <div key={upd.name} style={{ padding: '12px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '6px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <div>
                  <div style={{ fontSize: '13px', fontWeight: 500 }}>{upd.name}</div>
                  <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)', marginTop: '2px' }}>Installé le {upd.date} · {upd.size}</div>
                </div>
                <span style={{ fontSize: '11px', padding: '3px 10px', borderRadius: '10px', backgroundColor: upd.type === 'Sécurité' ? 'rgba(239,68,68,0.12)' : 'var(--win-accent-light)', color: upd.type === 'Sécurité' ? '#ef4444' : 'var(--win-accent)', fontWeight: 600, flexShrink: 0 }}>{upd.type}</span>
              </div>
            ))}

            <SectionTitle>Options avancées</SectionTitle>
            <SettingRow icon="clock" title="Mises à jour actives" desc="Choisissez quand les mises à jour sont installées" toggle={autoUpdate} onToggle={() => setAutoUpdate(!autoUpdate)} />
            <SettingRow icon="sliders" title="Heures actives" desc="Prévenir les redémarrages entre 08:00 et 23:00" onClick={() => addNotification('Mise à jour', 'Heures actives', 'Paramètres', 'sliders')} badge="08:00 – 23:00" />
            <SettingRow icon="sliders" title="Mises à jour optionnelles" desc="Pilotes et mises à jour non critiques" onClick={() => addNotification('Mise à jour', 'Mises à jour optionnelles', 'Paramètres', 'sliders')} />
            <SettingRow icon="sliders" title="Windows Insider Program" desc="Accéder aux versions préliminaires de Windows" onClick={() => addNotification('Mise à jour', 'Windows Insider Program', 'Paramètres', 'sliders')} />
          </div>
        )}

        {/* ABOUT */}
        {section === 'about' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>À propos</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Spécifications de l'appareil et de Windows 11</p>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '16px' }}></div>

            {hardwareInfo && <div role="status" style={{ padding: 14, borderRadius: 10, background: 'var(--flyout-card)', marginBottom: 16, fontSize: 13 }}>
              <div><strong>{hardwareInfo.cpuName || 'Processeur non détecté'}</strong></div>
              <div>{hardwareInfo.gpuName || 'GPU non détecté'}</div>
              <div>{Number.isFinite(hardwareInfo.ramTotalGB) ? Math.round(hardwareInfo.ramTotalGB) + ' Go' : 'RAM non détectée'}</div>
              {(hardwareInfo.disks || []).map((disk: any) => <div key={disk.name}>{disk.name} · {disk.format}</div>)}
            </div>}
            {/* Device banner */}
            <div style={{ padding: '24px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(0,120,212,0.12), rgba(139,92,246,0.12))', border: '1px solid rgba(0,120,212,0.35)', marginBottom: '20px', display: 'flex', alignItems: 'center', gap: '20px' }}>
              <FluentIcon name="pc" size={52} />
              <div>
                <div style={{ fontSize: '18px', fontWeight: 700 }}>DESKTOP-WIN11-PRO</div>
                <div style={{ fontSize: '13px', color: 'var(--text-secondary)', marginTop: '4px' }}>Windows 11 Professionnel · Build 26100.1742</div>
                <div style={{ fontSize: '12px', color: 'var(--text-tertiary)', marginTop: '2px' }}>AMD Ryzen 9 7950X · 32 Go DDR5 · GPU RTX 4090</div>
              </div>
            </div>

            <SectionTitle>Spécifications de l'appareil</SectionTitle>
            <div style={{ padding: '20px', borderRadius: '12px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '16px' }}>
              {[
                ['Nom de l\'appareil', 'DESKTOP-WIN11-PRO'],
                ['Processeur', 'AMD Ryzen 9 7950X 16-Core Processor 4.50 GHz'],
                ['Mémoire vive (RAM)', '32,0 Go (31,8 Go utilisable) DDR5-6000'],
                ['Carte graphique', 'NVIDIA GeForce RTX 4090 24 Go GDDR6X'],
                ['Stockage', 'Samsung 990 Pro NVMe 2 To (SSD)'],
                ['ID de périphérique', 'C4A2F3D8-1B5E-4F6A-9C2D-7E8B3F0A1C4D'],
                ['ID de produit', '00330-80000-00000-AAOEM'],
                ['Type de système', 'Système d\'exploitation 64 bits, processeur x64'],
                ['Stylet et entrée tactile', 'Ce PC possède une assistance tactile à 10 points'],
              ].map(([label, value]) => (
                <div key={label as string} style={{ display: 'grid', gridTemplateColumns: '200px 1fr', marginBottom: '10px', fontSize: '13px' }}>
                  <span style={{ color: 'var(--text-secondary)' }}>{label}</span>
                  <span style={{ fontWeight: 400 }}>{value}</span>
                </div>
              ))}
            </div>

            <SectionTitle>Spécifications de Windows</SectionTitle>
            <div style={{ padding: '20px', borderRadius: '12px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '16px' }}>
              {[
                ['Édition', 'Windows 11 Professionnel'],
                ['Version', '24H2'],
                ['Installé le', '31/10/2026'],
                ['Build du système d\'exploitation', '26100.1742'],
                ['Expérience', 'Windows Feature Experience Pack 1000.26100.32.0'],
              ].map(([label, value]) => (
                <div key={label as string} style={{ display: 'grid', gridTemplateColumns: '200px 1fr', marginBottom: '10px', fontSize: '13px' }}>
                  <span style={{ color: 'var(--text-secondary)' }}>{label}</span>
                  <span style={{ fontWeight: 400 }}>{value}</span>
                </div>
              ))}
            </div>

            <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
              {[
                'Renommer ce PC',
                'Paramètres système avancés',
                'Protection de l\'appareil',
                'Gestionnaire de périphériques',
                'Connexions réseau avancées',
              ].map(action => (
                <button key={action} style={{ padding: '8px 16px', borderRadius: '10px', border: '1px solid rgba(139,92,246,0.2)', background: 'rgba(139,92,246,0.06)', color: 'var(--text-primary)', fontSize: '13px', fontWeight: 600, cursor: 'pointer', transition: 'all 0.15s ease' }}
                  onMouseEnter={e => { e.currentTarget.style.background = 'rgba(139,92,246,0.12)'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.35)'; }}
                  onMouseLeave={e => { e.currentTarget.style.background = 'rgba(139,92,246,0.06)'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.2)'; }}
                >{action}</button>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
