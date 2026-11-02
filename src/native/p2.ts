// P2 helpers — stockage, affichage multi-écrans, applications installées.
import { nativeBridge } from './NativeBridge'

export type StorageVolume = {
  name: string
  label: string
  driveType: string
  format: string
  isReady: boolean
  totalBytes: number
  freeBytes: number
  usedBytes: number
  freePercent: number
}

export type StorageSnapshot = {
  volumes: StorageVolume[]
  disks: { model: string; interfaceType: string; mediaType: string; sizeBytes: number; serial: string; status: string }[]
  volumeCount: number
  diskCount: number
  source: string
  note: string
}

export type DisplayMonitor = {
  index: number
  device: string
  isPrimary: boolean
  bounds: { x: number; y: number; width: number; height: number }
  workArea: { x: number; y: number; width: number; height: number }
  resolution: string
}

export type DisplaySnapshot = {
  resolution: string
  width: number
  height: number
  monitorCount: number
  virtualScreen: { x: number; y: number; width: number; height: number }
  monitors: DisplayMonitor[]
  source: string
  note: string
}

export type InstalledApp = {
  id: string
  name: string
  version: string
  publisher: string
  installLocation: string
  hasUninstaller: boolean
  installDate: string
}

export type AppsSnapshot = {
  apps: InstalledApp[]
  count: number
  truncated: boolean
  totalFound: number
  source: string
  note: string
}

/** Volumes et disques Windows (lecture seule). */
export async function readStorageVolumes(): Promise<StorageSnapshot> {
  const response = await nativeBridge.send('storage/volumes', {})
  if (!response.result) throw new Error('Aucun inventaire de stockage.')
  return response.result as StorageSnapshot
}

/** Multi-écrans (User32). */
export async function readDisplayLayout(): Promise<DisplaySnapshot> {
  const response = await nativeBridge.send('display/read', {})
  if (!response.result) throw new Error('Aucun inventaire d\'affichage.')
  return response.result as DisplaySnapshot
}

/** Applications installées (registre Uninstall). */
export async function readInstalledApps(maxItems = 400): Promise<AppsSnapshot> {
  const response = await nativeBridge.send('apps/installed', { maxItems })
  if (!response.result) throw new Error('Aucun inventaire d\'applications.')
  return response.result as AppsSnapshot
}
