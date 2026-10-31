import React, { useState, useEffect } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

export const LockScreen: React.FC = () => {
  const { isLocked, isSleeping, unlockOS, wakeOS, wallpaper, restartOS, shutdownOS } = useOS()
  const [showPinInput, setShowPinInput] = useState(false)
  const [pin, setPin] = useState('')
  const [currentTime, setCurrentTime] = useState(new Date())

  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(new Date()), 1000)
    return () => clearInterval(timer)
  }, [])

  if (isSleeping) {
    return (
      <div
        onClick={wakeOS}
        style={{
          position: 'fixed',
          inset: 0,
          backgroundColor: '#000000',
          zIndex: 9999999,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          cursor: 'pointer',
        }}
      >
        <span style={{ color: '#666666', fontSize: '13px', animation: 'pulseGlow 2s infinite' }}>
          Appuyez n'importe où pour sortir du mode veille
        </span>
      </div>
    )
  }

  if (!isLocked) return null

  const formattedTime = currentTime.toLocaleTimeString('fr-FR', {
    hour: '2-digit',
    minute: '2-digit',
  })

  const formattedDate = currentTime.toLocaleDateString('fr-FR', {
    weekday: 'long',
    day: 'numeric',
    month: 'long',
  })

  const handleUnlock = (e: React.FormEvent) => {
    e.preventDefault()
    if (pin === '1234') {
      unlockOS()
      setShowPinInput(false)
      setPin('')
    } else if (pin === '') {
      setPin('')
    } else {
      setPin('')
    }
  }

  return (
    <div
      onClick={() => {
        if (!showPinInput) setShowPinInput(true)
      }}
      style={{
        position: 'fixed',
        inset: 0,
        backgroundImage: `url(${wallpaper.url})`,
        backgroundSize: 'cover',
        backgroundPosition: 'center',
        zIndex: 999999,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'space-between',
        padding: '60px 40px',
        color: '#ffffff',
        cursor: showPinInput ? 'default' : 'pointer',
        animation: 'windowOpen 0.2s ease',
      }}
    >
      {/* Dim overlay when password/PIN active */}
      {showPinInput && (
        <div
          style={{
            position: 'absolute',
            inset: 0,
            backgroundColor: 'rgba(0, 0, 0, 0.45)',
            backdropFilter: 'blur(20px)',
            WebkitBackdropFilter: 'blur(20px)',
            zIndex: 1,
          }}
        />
      )}

      {/* Top Header / Clock */}
      {!showPinInput ? (
        <div
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            marginTop: '80px',
            textShadow: '0 2px 10px rgba(0,0,0,0.5)',
            zIndex: 2,
          }}
        >
          <div style={{ fontSize: '84px', fontWeight: 600, letterSpacing: '-2px', lineHeight: '1' }}>
            {formattedTime}
          </div>
          <div style={{ fontSize: '20px', fontWeight: 400, marginTop: '12px', textTransform: 'capitalize' }}>
            {formattedDate}
          </div>
        </div>
      ) : (
        /* Pin / Password Form */
        <div
          onClick={e => e.stopPropagation()}
          style={{
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            margin: 'auto 0',
            zIndex: 2,
          }}
        >
          <div
            style={{
              width: '88px',
              height: '88px',
              borderRadius: '50%',
              backgroundColor: 'var(--win-accent)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              fontSize: '36px',
              fontWeight: 600,
              boxShadow: '0 8px 24px rgba(0,0,0,0.4)',
              marginBottom: '16px',
            }}
          >
            U
          </div>

          <h2 style={{ fontSize: '24px', fontWeight: 600, marginBottom: '20px' }}>
            Utilisateur Windows
          </h2>

          <form
            onSubmit={handleUnlock}
            style={{
              display: 'flex',
              alignItems: 'center',
              backgroundColor: 'rgba(255, 255, 255, 0.15)',
              borderRadius: '6px',
              border: '1px solid rgba(255, 255, 255, 0.3)',
              overflow: 'hidden',
              width: '260px',
            }}
          >
            <input
              autoFocus
              type="password"
              placeholder="Code PIN ou mot de passe"
              value={pin}
              onChange={e => setPin(e.target.value)}
              style={{
                flex: 1,
                padding: '10px 14px',
                background: 'transparent',
                border: 'none',
                outline: 'none',
                color: '#ffffff',
                fontSize: '13px',
              }}
            />
            <button
              type="submit"
              style={{
                padding: '10px 14px',
                background: 'transparent',
                border: 'none',
                color: '#ffffff',
                cursor: 'pointer',
              }}
            >
              <FluentIcon name="arrow-right" size={16} />
            </button>
          </form>

          <span
            onClick={() => setShowPinInput(false)}
            style={{
              fontSize: '12.5px',
              color: 'rgba(255, 255, 255, 0.7)',
              marginTop: '16px',
              cursor: 'pointer',
            }}
          >
            Retour à l'écran de verrouillage
          </span>
        </div>
      )}

      {/* Bottom Hint or Power options */}
      <div
        style={{
          width: '100%',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'flex-end',
          zIndex: 2,
        }}
      >
        <div style={{ fontSize: '13px', color: 'rgba(255, 255, 255, 0.8)' }}>
          {!showPinInput && 'Cliquez pour vous connecter'}
        </div>

        <div style={{ display: 'flex', gap: '12px' }}>
          <button
            onClick={e => {
              e.stopPropagation()
              restartOS()
            }}
            title="Redémarrer"
            style={{
              padding: '10px',
              borderRadius: '50%',
              border: 'none',
              backgroundColor: 'rgba(0, 0, 0, 0.4)',
              color: '#ffffff',
              cursor: 'pointer',
            }}
          >
            <FluentIcon name="restart" size={16} />
          </button>
          <button
            onClick={e => {
              e.stopPropagation()
              shutdownOS()
            }}
            title="Arrêter"
            style={{
              padding: '10px',
              borderRadius: '50%',
              border: 'none',
              backgroundColor: 'rgba(0, 0, 0, 0.4)',
              color: '#ffffff',
              cursor: 'pointer',
            }}
          >
            <FluentIcon name="power" size={16} />
          </button>
        </div>
      </div>
    </div>
  )
}
