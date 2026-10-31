import React from 'react'
import {
  Wifi,
  WifiOff,
  Volume2,
  VolumeX,
  Battery,
  BatteryCharging,
  Bell,
  Sun,
  Moon,
  Power,
  RotateCw,
  Lock,
  ChevronUp,
  ChevronRight,
  ChevronDown,
  ChevronLeft,
  X,
  Minus,
  Square,
  Copy,
  Plus,
  RefreshCw,
  ArrowLeft,
  ArrowRight,
  ArrowUp,
  Sliders,
  Check,
  Maximize2,
  Minimize2,
  HelpCircle,
  Clock,
  Star,
  FileText,
  Image as ImageIcon,
  Info,
  Play,
  Pause,
  Globe,
} from 'lucide-react'

interface FluentIconProps {
  name: string
  size?: number
  className?: string
  color?: string
  style?: React.CSSProperties
}

export const FluentIcon: React.FC<FluentIconProps> = ({
  name,
  size = 20,
  className = '',
  color,
  style,
}) => {
  const norm = name.toLowerCase()

  // 1. Windows 11 This PC / Computer (Authentic 3D Monitor)
  if (norm === 'pc' || norm === 'this-pc') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="pcScreen" x1="4" y1="4" x2="44" y2="34" gradientUnits="userSpaceOnUse">
            <stop stopColor="#0078D4" />
            <stop offset="1" stopColor="#002050" />
          </linearGradient>
          <linearGradient id="pcStand" x1="18" y1="36" x2="30" y2="44" gradientUnits="userSpaceOnUse">
            <stop stopColor="#94A3B8" />
            <stop offset="1" stopColor="#475569" />
          </linearGradient>
        </defs>
        {/* Monitor Frame */}
        <rect x="4" y="5" width="40" height="28" rx="4" fill="#1E293B" stroke="#334155" strokeWidth="1.5" />
        {/* Screen with Bloom Wallpaper */}
        <rect x="7" y="8" width="34" height="22" rx="2" fill="url(#pcScreen)" />
        {/* Windows Bloom flower hint */}
        <path d="M24 12c-3 4-2 9 0 11 2-2 3-7 0-11z" fill="#38BDF8" fillOpacity="0.8" />
        <path d="M19 19c4-3 9-2 11 0-2 2-7 3-11 0z" fill="#818CF8" fillOpacity="0.8" />
        {/* Stand Base */}
        <path d="M21 33h6v5h-6z" fill="url(#pcStand)" />
        <rect x="14" y="38" width="20" height="3" rx="1.5" fill="#64748B" />
      </svg>
    )
  }

  // 2. Windows 11 Recycle Bin (Corbeille 3D Translucent)
  if (norm === 'trash' || norm === 'recycle' || norm === 'corbeille') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="binBody" x1="12" y1="16" x2="36" y2="42" gradientUnits="userSpaceOnUse">
            <stop stopColor="#38BDF8" stopOpacity="0.85" />
            <stop offset="1" stopColor="#0284C7" stopOpacity="0.95" />
          </linearGradient>
          <linearGradient id="binRim" x1="8" y1="10" x2="40" y2="16" gradientUnits="userSpaceOnUse">
            <stop stopColor="#BAE6FD" />
            <stop offset="1" stopColor="#0284C7" />
          </linearGradient>
        </defs>
        {/* Bin Body Cylinder */}
        <path
          d="M13 15h22l-2.5 24.5a3 3 0 01-3 2.5h-11a3 3 0 01-3-2.5L13 15z"
          fill="url(#binBody)"
          stroke="#7DD3FC"
          strokeWidth="1.2"
        />
        {/* Vertical ribs */}
        <path d="M19 18v19M24 18v19M29 18v19" stroke="#E0F2FE" strokeWidth="1" strokeOpacity="0.5" strokeLinecap="round" />
        {/* Bin Top Rim Ellipse */}
        <ellipse cx="24" cy="14" rx="13" ry="3.5" fill="url(#binRim)" stroke="#FFFFFF" strokeWidth="1" />
        {/* Recycling arrows symbol */}
        <path
          d="M24 23l2 3h-1.5a3 3 0 01-3-3v-.5l-1.5 1.5M26.5 27.5l-2-3h1.5a3 3 0 013 3v.5l1.5-1.5"
          stroke="#FFFFFF"
          strokeWidth="1.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    )
  }

  // 3. Windows 11 File Explorer / Folder (Official Manila + Blue Tab)
  if (norm === 'explorer' || norm === 'folder') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="fldBack" x1="6" y1="10" x2="42" y2="38" gradientUnits="userSpaceOnUse">
            <stop stopColor="#0284C7" />
            <stop offset="1" stopColor="#0369A1" />
          </linearGradient>
          <linearGradient id="fldFront" x1="6" y1="17" x2="42" y2="40" gradientUnits="userSpaceOnUse">
            <stop stopColor="#FBBF24" />
            <stop offset="1" stopColor="#D97706" />
          </linearGradient>
          <linearGradient id="fldPaper" x1="12" y1="11" x2="36" y2="28" gradientUnits="userSpaceOnUse">
            <stop stopColor="#FFFFFF" />
            <stop offset="1" stopColor="#E2E8F0" />
          </linearGradient>
        </defs>
        {/* Blue Folder Back with Tab */}
        <path d="M6 13a3 3 0 013-3h10l3.5 3.5H39a3 3 0 013 3v20a3 3 0 01-3 3H9a3 3 0 01-3-3V13z" fill="url(#fldBack)" />
        {/* White Document Paper inside */}
        <rect x="11" y="13" width="26" height="15" rx="2" fill="url(#fldPaper)" />
        <line x1="15" y1="17" x2="25" y2="17" stroke="#94A3B8" strokeWidth="1.5" strokeLinecap="round" />
        <line x1="15" y1="21" x2="30" y2="21" stroke="#94A3B8" strokeWidth="1.5" strokeLinecap="round" />
        {/* Yellow Manila Front Flap */}
        <path
          d="M6 19a3 3 0 013-3h30a3 3 0 013 3v17a3 3 0 01-3 3H9a3 3 0 01-3-3V19z"
          fill="url(#fldFront)"
          stroke="#F59E0B"
          strokeWidth="0.8"
        />
      </svg>
    )
  }

  // 4. Microsoft Edge (Famous Blue-Green Wave Surf Logo)
  if (norm === 'globe') return <Globe width={size} height={size} color={color} className={className} style={style} />


  if (norm === 'notepad' || norm === 'file-text') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="padGrad" x1="8" y1="6" x2="36" y2="42" gradientUnits="userSpaceOnUse">
            <stop stopColor="#3B82F6" />
            <stop offset="1" stopColor="#1D4ED8" />
          </linearGradient>
          <linearGradient id="padPaper" x1="12" y1="10" x2="34" y2="38" gradientUnits="userSpaceOnUse">
            <stop stopColor="#FFFFFF" />
            <stop offset="1" stopColor="#F1F5F9" />
          </linearGradient>
        </defs>
        {/* Notebook Cover */}
        <rect x="7" y="6" width="30" height="36" rx="4" fill="url(#padGrad)" />
        {/* Spine Rings */}
        <circle cx="9" cy="12" r="1.5" fill="#93C5FD" />
        <circle cx="9" cy="18" r="1.5" fill="#93C5FD" />
        <circle cx="9" cy="24" r="1.5" fill="#93C5FD" />
        <circle cx="9" cy="30" r="1.5" fill="#93C5FD" />
        <circle cx="9" cy="36" r="1.5" fill="#93C5FD" />
        {/* Inner Page */}
        <rect x="13" y="9" width="22" height="30" rx="2" fill="url(#padPaper)" />
        {/* Text lines */}
        <line x1="16" y1="15" x2="28" y2="15" stroke="#3B82F6" strokeWidth="1.5" strokeLinecap="round" />
        <line x1="16" y1="20" x2="31" y2="20" stroke="#94A3B8" strokeWidth="1.5" strokeLinecap="round" />
        <line x1="16" y1="25" x2="27" y2="25" stroke="#94A3B8" strokeWidth="1.5" strokeLinecap="round" />
        <line x1="16" y1="30" x2="29" y2="30" stroke="#94A3B8" strokeWidth="1.5" strokeLinecap="round" />
        {/* Pencil */}
        <path d="M33 28l6-6 3 3-6 6-3.5.5.5-3.5z" fill="#F59E0B" stroke="#D97706" strokeWidth="0.8" />
        <path d="M42 25l-2-2 1-1a1 1 0 011.4 0l.6.6a1 1 0 010 1.4l-1 1z" fill="#EF4444" />
      </svg>
    )
  }

  // 6. Windows Terminal (Sleek Dark Console with Cyan Prompt >_)
  if (norm === 'terminal') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="termBg" x1="4" y1="8" x2="44" y2="40" gradientUnits="userSpaceOnUse">
            <stop stopColor="#1E293B" />
            <stop offset="1" stopColor="#090D16" />
          </linearGradient>
        </defs>
        {/* Terminal Window Box */}
        <rect x="5" y="8" width="38" height="32" rx="5" fill="url(#termBg)" stroke="#334155" strokeWidth="1.5" />
        {/* Top Header bar */}
        <path d="M5 13a5 5 0 015-5h28a5 5 0 015 5v3H5v-3z" fill="#0F172A" />
        {/* Terminal window dots */}
        <circle cx="10" cy="11.5" r="1.5" fill="#EF4444" />
        <circle cx="15" cy="11.5" r="1.5" fill="#F59E0B" />
        <circle cx="20" cy="11.5" r="1.5" fill="#10B981" />
        {/* Prompt chevron and cursor */}
        <path d="M12 21l6 5-6 5" stroke="#00D4FF" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
        <rect x="22" y="28" width="10" height="2.5" rx="1" fill="#8B5CF6" />
      </svg>
    )
  }

  // 7. Windows 11 Calculator (Rounded Fluent Math Tile)
  if (norm === 'calculator') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="calcGrad" x1="8" y1="6" x2="40" y2="42" gradientUnits="userSpaceOnUse">
            <stop stopColor="#8B5CF6" />
            <stop offset="1" stopColor="#6D28D9" />
          </linearGradient>
        </defs>
        {/* Calculator Body */}
        <rect x="9" y="6" width="30" height="36" rx="6" fill="url(#calcGrad)" stroke="#A78BFA" strokeWidth="1" />
        {/* Display Screen */}
        <rect x="13" y="10" width="22" height="9" rx="3" fill="#1E1B4B" stroke="#4C1D95" strokeWidth="0.8" />
        <text x="31" y="17.5" fill="#00D4FF" fontSize="7" fontWeight="bold" textAnchor="end" fontFamily="monospace">
          137
        </text>
        {/* Keypad Grid 3x3 */}
        <rect x="13" y="22" width="5" height="4" rx="1.5" fill="#4C1D95" />
        <rect x="21" y="22" width="5" height="4" rx="1.5" fill="#4C1D95" />
        <rect x="29" y="22" width="5" height="4" rx="1.5" fill="#00D4FF" />
        <rect x="13" y="28" width="5" height="4" rx="1.5" fill="#4C1D95" />
        <rect x="21" y="28" width="5" height="4" rx="1.5" fill="#4C1D95" />
        <rect x="29" y="28" width="5" height="4" rx="1.5" fill="#A78BFA" />
        <rect x="13" y="34" width="5" height="4" rx="1.5" fill="#4C1D95" />
        <rect x="21" y="34" width="5" height="4" rx="1.5" fill="#4C1D95" />
        <rect x="29" y="34" width="5" height="4" rx="1.5" fill="#06B6D4" />
      </svg>
    )
  }

  // 8. Windows 11 Settings (Fluent Mechanical Depth Gears)
  if (norm === 'settings') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="gearMain" x1="10" y1="10" x2="38" y2="38" gradientUnits="userSpaceOnUse">
            <stop stopColor="#64748B" />
            <stop offset="1" stopColor="#334155" />
          </linearGradient>
          <linearGradient id="gearBlue" x1="16" y1="16" x2="32" y2="32" gradientUnits="userSpaceOnUse">
            <stop stopColor="#00D4FF" />
            <stop offset="1" stopColor="#0284C7" />
          </linearGradient>
        </defs>
        {/* Large Gear Outer with 8 teeth */}
        <circle cx="24" cy="24" r="14" fill="url(#gearMain)" stroke="#94A3B8" strokeWidth="1" />
        <path
          d="M22 6h4v4h-4zM22 38h4v4h-4zM6 22h4v4H6zM38 22h4v4h-4zM11.5 11.5l3 3-2.8 2.8-3-3zM33.5 33.5l3 3-2.8 2.8-3-3zM33.5 14.5l3-3 2.8 2.8-3 3zM11.5 36.5l3-3 2.8 2.8-3 3z"
          fill="#475569"
        />
        {/* Center blue cog core */}
        <circle cx="24" cy="24" r="7" fill="url(#gearBlue)" stroke="#FFFFFF" strokeWidth="1.2" />
        <circle cx="24" cy="24" r="3" fill="#0A0A12" />
      </svg>
    )
  }

  // 9. Windows 11 Paint (Artist Palette with Rainbow Paint & Brush)
  if (norm === 'paint' || norm === 'palette') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="palBody" x1="8" y1="10" x2="38" y2="40" gradientUnits="userSpaceOnUse">
            <stop stopColor="#FDE68A" />
            <stop offset="1" stopColor="#F59E0B" />
          </linearGradient>
        </defs>
        {/* Palette Board */}
        <path
          d="M24 6C13 6 5 14 5 25c0 8.5 6.5 15 15 15 2.5 0 4-1.5 4-3.5 0-1-.5-2-.5-3 0-1.5 1.5-3 3-3h3.5c7 0 13-6 13-13C43 14 34.5 6 24 6z"
          fill="url(#palBody)"
          stroke="#D97706"
          strokeWidth="1"
        />
        {/* Thumb Hole */}
        <ellipse cx="33" cy="28" rx="3.5" ry="4.5" fill="#0A0A12" opacity="0.65" />
        {/* Colorful paint blobs */}
        <circle cx="14" cy="18" r="3" fill="#EF4444" />
        <circle cx="22" cy="13" r="3" fill="#F59E0B" />
        <circle cx="31" cy="14" r="3" fill="#10B981" />
        <circle cx="15" cy="28" r="3" fill="#3B82F6" />
        <circle cx="22" cy="33" r="2.5" fill="#8B5CF6" />
      </svg>
    )
  }

  // 10. Copilot (Gradient 3D Ribbon Loop)
  if (norm === 'copilot' || norm === 'sparkles') {
    return (
      <svg width={size} height={size} viewBox="0 0 48 48" fill="none" className={className} style={style}>
        <defs>
          <linearGradient id="copilotA" x1="6" y1="12" x2="38" y2="38" gradientUnits="userSpaceOnUse">
            <stop stopColor="#38BDF8" />
            <stop offset="0.5" stopColor="#818CF8" />
            <stop offset="1" stopColor="#C084FC" />
          </linearGradient>
          <linearGradient id="copilotB" x1="42" y1="12" x2="10" y2="38" gradientUnits="userSpaceOnUse">
            <stop stopColor="#FB7185" />
            <stop offset="0.5" stopColor="#F472B6" />
            <stop offset="1" stopColor="#38BDF8" />
          </linearGradient>
        </defs>
        <path
          d="M17 12C12 12 8 16 8 21c0 6.5 5 11 11 11h10c6 0 11-4.5 11-11 0-5-4-9-9-9-3 0-6 1.5-8 4-2-2.5-5-4-8-4z"
          stroke="url(#copilotA)"
          strokeWidth="5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
        <path
          d="M31 36c5 0 9-4 9-9 0-6.5-5-11-11-11H19c-6 0-11 4.5-11 11 0 5 4 9 9 9 3 0 6-1.5 8-4 2 2.5 5 4 8 4z"
          stroke="url(#copilotB)"
          strokeWidth="5"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </svg>
    )
  }

  // Standard icon fallbacks with Lucide
  const props = { size, className, color, style }
  switch (norm) {
    case 'search':
      return (
        <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={color || 'currentColor'} strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
      )
    case 'wifi':
      return <Wifi {...props} />
    case 'wifi-off':
      return <WifiOff {...props} />
    case 'volume':
      return <Volume2 {...props} />
    case 'volume-x':
      return <VolumeX {...props} />
    case 'battery':
      return <Battery {...props} />
    case 'battery-charging':
      return <BatteryCharging {...props} />
    case 'bell':
      return <Bell {...props} />
    case 'sun':
      return <Sun {...props} />
    case 'moon':
      return <Moon {...props} />
    case 'power':
      return <Power {...props} />
    case 'restart':
      return <RotateCw {...props} />
    case 'lock':
      return <Lock {...props} />
    case 'chevron-up':
      return <ChevronUp {...props} />
    case 'chevron-down':
      return <ChevronDown {...props} />
    case 'chevron-left':
      return <ChevronLeft {...props} />
    case 'chevron-right':
      return <ChevronRight {...props} />
    case 'close':
    case 'x':
      return <X {...props} />
    case 'minus':
      return <Minus {...props} />
    case 'square':
      return <Square {...props} />
    case 'copy':
      return <Copy {...props} />
    case 'plus':
      return <Plus {...props} />
    case 'refresh':
      return <RefreshCw {...props} />
    case 'arrow-left':
      return <ArrowLeft {...props} />
    case 'arrow-right':
      return <ArrowRight {...props} />
    case 'arrow-up':
      return <ArrowUp {...props} />
    case 'sliders':
      return <Sliders {...props} />
    case 'star':
      return <Star {...props} />
    case 'file-text':
      return <FileText {...props} />
    case 'image':
    case 'photos':
      return <ImageIcon {...props} />
    case 'info':
      return <Info {...props} />
    case 'play':
      return <Play {...props} />
    case 'pause':
      return <Pause {...props} />
    case 'check':
      return <Check {...props} />
    case 'maximize':
      return <Maximize2 {...props} />
    case 'minimize':
      return <Minimize2 {...props} />
    case 'clock':
      return <Clock {...props} />
    default:
      return <HelpCircle {...props} />
  }
}

/**
 * Windows 11 Official 4-Tiles Gradient Logo
 */
export const WindowsLogo: React.FC<{ size?: number; className?: string; style?: React.CSSProperties }> = ({
  size = 22,
  className = '',
  style,
}) => {
  return (
    <svg width={size} height={size} viewBox="0 0 88 88" fill="none" className={className} style={style}>
      <defs>
        <linearGradient id="winLogoGrad" x1="0" y1="0" x2="88" y2="88" gradientUnits="userSpaceOnUse">
          <stop stopColor="#00D4FF" />
          <stop offset="0.6" stopColor="#0078D4" />
          <stop offset="1" stopColor="#8B5CF6" />
        </linearGradient>
      </defs>
      <rect x="2" y="2" width="39" height="39" rx="3" fill="url(#winLogoGrad)" />
      <rect x="47" y="2" width="39" height="39" rx="3" fill="url(#winLogoGrad)" />
      <rect x="2" y="47" width="39" height="39" rx="3" fill="url(#winLogoGrad)" />
      <rect x="47" y="47" width="39" height="39" rx="3" fill="url(#winLogoGrad)" />
    </svg>
  )
}
