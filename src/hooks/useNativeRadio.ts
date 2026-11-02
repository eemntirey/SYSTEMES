import { useCallback, useEffect, useRef } from 'react'
import { NativeBridge } from '../services/NativeBridge'

type Notify = (title: string, message: string, appName: string, icon?: string) => void

/**
 * Contrôle les adaptateurs physiques via le moteur natif Windows.
 * Aucun état optimiste : l'interface ne change qu'après confirmation de Windows.
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
    void NativeBridge.readWifiAdapterStatus()
      .then(status => { if (!cancelled) setWifiEnabled(status.enabled) })
      .catch(() => { /* Moteur natif absent : ne pas fabriquer un état radio. */ })
    void NativeBridge.readBluetoothStatus()
      .then(status => { if (!cancelled && !status.error) setBluetoothEnabled(!!status.powered) })
      .catch(() => { /* Moteur natif absent. */ })
    return () => {
      cancelled = true
      mounted.current = false
    }
  }, [setWifiEnabled, setBluetoothEnabled])

  const refreshWifiStatus = useCallback(async () => {
    const status = await NativeBridge.readWifiAdapterStatus()
    if (mounted.current) setWifiEnabled(status.enabled)
    return status
  }, [setWifiEnabled])

  const refreshBluetoothStatus = useCallback(async () => {
    const status = await NativeBridge.readBluetoothStatus()
    if (status.error) throw new Error(status.error)
    if (mounted.current) setBluetoothEnabled(!!status.powered)
    return status
  }, [setBluetoothEnabled])

  const toggleWifi = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readWifiAdapterStatus()
        const result = await NativeBridge.setWifiEnabled(!current.enabled)
        if (mounted.current) setWifiEnabled(result.enabled)
        addNotification('Wi-Fi', result.enabled
          ? `Adaptateur « ${result.adapterName} » activé.`
          : `Adaptateur « ${result.adapterName} » désactivé.`, 'Réseau')
      } catch (cause) {
        addNotification('Wi-Fi — opération refusée',
          cause instanceof Error ? cause.message : String(cause), 'Réseau')
      }
    })()
  }, [addNotification, setWifiEnabled])

  const toggleBluetooth = useCallback(() => {
    void (async () => {
      try {
        const current = await NativeBridge.readBluetoothStatus()
        if (current.error) throw new Error(current.error)
        const result = await NativeBridge.setBluetoothEnabled(!current.powered)
        if (mounted.current) setBluetoothEnabled(result.enabled)
        addNotification('Bluetooth', result.enabled
          ? `Adaptateur « ${result.adapterName} » activé.`
          : `Adaptateur « ${result.adapterName} » désactivé.`, 'Bluetooth')
      } catch (cause) {
        addNotification('Bluetooth — opération refusée',
          cause instanceof Error ? cause.message : String(cause), 'Bluetooth')
      }
    })()
  }, [addNotification, setBluetoothEnabled])

  return { toggleWifi, toggleBluetooth, refreshWifiStatus, refreshBluetoothStatus }
}
