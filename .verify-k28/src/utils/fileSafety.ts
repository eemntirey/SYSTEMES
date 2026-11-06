/**
 * Garde-fous côté interface pour les opérations fichiers SYSTEMES.
 * Complète les contrôles déjà présents dans FileSystemNative.cs
 * (symlinks, ADS, device paths, FileMode.CreateNew, Corbeille).
 */

const RESERVED_WINDOWS_NAMES = new Set([
  'CON', 'PRN', 'AUX', 'NUL',
  'COM1', 'COM2', 'COM3', 'COM4', 'COM5', 'COM6', 'COM7', 'COM8', 'COM9',
  'LPT1', 'LPT2', 'LPT3', 'LPT4', 'LPT5', 'LPT6', 'LPT7', 'LPT8', 'LPT9',
])

/** Noms de fichiers/dossiers saisis par l'utilisateur (pas de chemin complet). */
export function validateEntryName(name: string): string | null {
  const trimmed = name.trim()
  if (!trimmed) return 'Le nom ne peut pas être vide.'
  if (trimmed.length > 255) return 'Le nom est trop long (255 caractères max).'
  if (/[<>:"/\\|?*\u0000-\u001f]/.test(trimmed)) {
    return 'Le nom contient des caractères interdits (< > : " / \\ | ? * ou contrôle).'
  }
  if (trimmed === '.' || trimmed === '..') return 'Les noms « . » et « .. » sont interdits.'
  if (trimmed.endsWith('.') || trimmed.endsWith(' ')) {
    return 'Un nom Windows ne peut pas se terminer par un point ou un espace.'
  }
  const base = trimmed.split('.')[0].toUpperCase()
  if (RESERVED_WINDOWS_NAMES.has(base)) {
    return `« ${trimmed} » est un nom réservé Windows.`
  }
  return null
}

/** Indique si le chemin semble sensible (système / racine volume). */
export function isSensitivePath(path: string): boolean {
  const normalized = path.replace(/\//g, '\\').toLowerCase()
  const sensitive = [
    '\\windows\\',
    '\\program files\\',
    '\\program files (x86)\\',
    '\\programdata\\',
    '\\$recycle.bin',
    '\\system volume information',
    '\\users\\all users',
    '\\users\\default',
    '\\users\\public\\desktop',
  ]
  if (sensitive.some(s => normalized.includes(s))) return true
  if (/^[a-z]:\\?$/i.test(normalized.replace(/\\+$/, ''))) return true
  return false
}

export type ConfirmKind = 'delete' | 'rename' | 'create-file' | 'create-folder'

export function confirmFileAction(
  kind: ConfirmKind,
  opts: { name: string; path?: string; newName?: string },
): boolean {
  const { name, path, newName } = opts

  if (kind === 'delete') {
    const location = path ? `\n\nChemin : ${path}` : ''
    const sensitive = path && isSensitivePath(path)
      ? '\n\n⚠ Cet emplacement semble sensible (système ou racine). Vérifiez avant de confirmer.'
      : ''
    return window.confirm(
      `Envoyer « ${name} » vers la Corbeille Windows ?${location}${sensitive}\n\nCette action peut être annulée depuis la Corbeille.`,
    )
  }

  if (kind === 'rename') {
    if (!newName) return false
    return window.confirm(
      `Renommer « ${name} » en « ${newName} » ?${path ? `\n\nChemin actuel : ${path}` : ''}`,
    )
  }

  if (kind === 'create-file' || kind === 'create-folder') {
    const label = kind === 'create-file' ? 'fichier' : 'dossier'
    return window.confirm(
      `Créer le ${label} « ${name} » ici ?${path ? `\n\nDossier parent : ${path}` : ''}`,
    )
  }

  return false
}

/** Message si un élément du même nom existe déjà dans la vue courante. */
export function existingNameConflict(
  name: string,
  items: { name: string }[],
): string | null {
  const hit = items.find(i => i.name.toLowerCase() === name.trim().toLowerCase())
  if (!hit) return null
  return `« ${hit.name} » existe déjà dans ce dossier.`
}
