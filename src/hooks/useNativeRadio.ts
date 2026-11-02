import { useCallback, useEffect, useRef } from 'react'
import { NativeBridge } from '../services/NativeBridge'

type Notify = (title: string, message: string, appName: string, icon?: string) => void

/**
 * Synchronise les interrupteurs Wi-Fi / Bluetooth du bureau avec le moteur natif Windows.
 * - Lecture réelle au montage (netsh / Get-PnpDevice)
 * - toggleWifi : déconnexion native si connecté, sinon ouverture des paramètres réseau
 * - toggleBluetooth : ouverture des paramètres Bluetooth + relecture de l'état réel
 * Hors host WebView2, retombe sur un basculement local (aucun faux succès silencieux).
 */
export function useNativeRadio(
  setWifiEnabled: (value: boolean | ((prev: boolean) => boolean)) => void,
  setBluetoothEnabled: (value: boolean | ((prev: boolean) => boolean)) => void,
  addNotification: Notify,
) {
  const mounted = useRef(true)

  useEffect(() => {
    mounted.current = true
    let cancelled = false
    ;(async () => {
      try {
        const wifi = await NativeBridge.readWifiStatus()
        if (!cancelled && wifi && !wifi.error) {
          setWifiEnabled(!!wifi.connected || (wifi.state != null && wifi.state !== ''))
        }
      } catch {
        /* hors host : conserver la préférence locale */
      }
      try {
        const bt = await NativeBridge.readBluetoothStatus()
        if (!cancelled && bt && !bt.error) {
          setBluetoothEnabled(!!bt.powered)
        }
      } catch {
        /* hors host */
      }
    })()
    return () => {
      cancelled = true
      mounted.current = false
    }
  }, [setWifiEnabled, setBluetoothEnabled])

  const refreshWifiStatus = useCallback(async () => {
    try {
      const wifi = await NativeBridge.readWifiStatus()
      if (wifi?.error) return wifi
      if (mounted.current) {
        setWifiEnabled(!!wifi?.connected || (wifi?.state != null && wifi.state !== ''))
      }
      return wifi
    } catch {
      return null
    }
  }, [setWifiEnabled])

  const refreshBluetoothStatus = useCallback(async () => {
    try {
      const bt = await NativeBridge.readBluetoothStatus()
      if (bt?.error) return bt
      if (mounted.current) {
        setBluetoothEnabled(!!bt?.powered)
      }
      return bt
    } catch {
      return null
    }
  }, [setBluetoothEnabled])

  const toggleWifi = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readWifiStatus()
        if (current?.connected) {
          await NativeBridge.disconnectWifi()
          await refreshWifiStatus()
          addNotification(
            'Wi-Fi',
            current.ssid ? `Déconnecté de « ${current.ssid} ».` : 'Réseau Wi-Fi déconnecté.',
            'Réseau',
          )
        } else {
          setWifiEnabled(true)
          try {
            await NativeBridge.openWindowsSettings('network-status')
          } catch {
            /* ignore hors host */
          }
          addNotification(
            'Wi-Fi',
            'Ouvrez les paramètres Windows pour vous connecter à un réseau.',
            'Réseau',
          )
          setTimeout(() => {
            void refreshWifiStatus()
          }, 2500)
        }
      } catch {
        setWifiEnabled(prev => !prev)
        addNotification(
          'Wi-Fi',
          'Moteur natif indisponible — état local uniquement.',
          'Réseau',
        )
      }
    })()
  }, [refreshWifiStatus, addNotification, setWifiEnabled])

  const toggleBluetooth = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readBluetoothStatus()
        if (current?.error) throw new Error(current.error)
        const wasPowered = !!current?.powered
        setBluetoothEnabled(!wasPowered)
        try {
          await NativeBridge.openWindowsSettings('bluetooth')
        } catch {
          /* ignore hors host */
        }
        addNotification(
          'Bluetooth',
          wasPowered
            ? 'Paramètres Bluetooth ouverts — désactivez l’adaptateur dans Windows si besoin.'
            : 'Paramètres Bluetooth ouverts — activez l’adaptateur dans Windows.',
          'Bluetooth',
        )
        setTimeout(() => {
          void refreshBluetoothStatus()
        }, 2500)
      } catch {
        setBluetoothEnabled(prev => !prev)
        addNotification(
          'Bluetooth',
          'Moteur natif indisponible — état local uniquement.',
          'Bluetooth',
        )
      }
    })()
  }, [refreshBluetoothStatus, addNotification, setBluetoothEnabled])

  return { toggleWifi, toggleBluetooth, refreshWifiStatus, refreshBluetoothStatus }
}
