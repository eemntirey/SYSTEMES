/**
 * Services Windows — lecture + start/stop (hôte natif, droits admin souvent requis).
 */
import { nativeBridge } from './NativeBridge'

export type ServiceInfo = {
  name: string
  displayName: string
  state: string
  startMode: string
  account: string
  processId: number
  executablePath: string
  protectedService?: boolean
}

export type ServicesSnapshot = {
  services: ServiceInfo[]
  count: number
  source: string
  note?: string
}

export type ServiceControlResult = {
  changed: boolean
  name: string
  displayName?: string
  state: string
  previousState?: string
  action: 'start' | 'stop'
  note?: string
  source?: string
  wmiCode?: number
}

export async function readServices(): Promise<ServicesSnapshot> {
  const response = await nativeBridge.send('services/read', {})
  if (!response.result) throw new Error('Aucun inventaire de services.')
  return response.result as ServicesSnapshot
}

export async function controlService(
  name: string,
  action: 'start' | 'stop',
): Promise<ServiceControlResult> {
  if (!name || !/^[A-Za-z0-9_.-]{1,128}$/.test(name)) {
    throw new Error('Nom de service invalide.')
  }
  const response = await nativeBridge.send('services/control', { name, action })
  if (!response.result) throw new Error(`Échec ${action} du service ${name}.`)
  return response.result as ServiceControlResult
}

export async function startService(name: string) {
  return controlService(name, 'start')
}

export async function stopService(name: string) {
  return controlService(name, 'stop')
}

/**
 * Test sûr : lit l’état, optionnellement start/stop d’un service non protégé.
 * Par défaut Spooler (impression) — impact limité si vous n’imprimez pas.
 */
export async function testServiceRoundTrip(
  name = 'Spooler',
  options: { startIfStopped?: boolean; stopIfRunning?: boolean; restore?: boolean } = {},
): Promise<{
  name: string
  before: string
  after?: string
  restored?: string
  steps: string[]
}> {
  const { startIfStopped = true, stopIfRunning = true, restore = true } = options
  const steps: string[] = []
  const snap = await readServices()
  const svc = snap.services.find(s => s.name.toLowerCase() === name.toLowerCase())
  if (!svc) throw new Error(`Service « ${name} » introuvable.`)
  if (svc.protectedService) throw new Error(`Service « ${name} » protégé — test refusé.`)

  const before = svc.state
  steps.push(`État initial : ${before}`)
  let after: string | undefined
  let restored: string | undefined

  if (before.toLowerCase() === 'running' && stopIfRunning) {
    const r = await stopService(name)
    after = r.state
    steps.push(`stop → ${r.state} (changed=${r.changed})`)
    if (restore) {
      const r2 = await startService(name)
      restored = r2.state
      steps.push(`restore start → ${r2.state}`)
    }
  } else if (before.toLowerCase() === 'stopped' && startIfStopped) {
    const r = await startService(name)
    after = r.state
    steps.push(`start → ${r.state} (changed=${r.changed})`)
    if (restore) {
      const r2 = await stopService(name)
      restored = r2.state
      steps.push(`restore stop → ${r2.state}`)
    }
  } else {
    steps.push('Aucun contrôle demandé pour cet état.')
  }

  return { name, before, after, restored, steps }
}
