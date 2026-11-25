/**
 * Saved versions are the host's checkpoints (Kit UI protocol section 7m): labelled snapshots of the
 * design's replica the host keeps beside the file. Nothing about them is in the `.kdesign` or the
 * replica. The editor lists, names and deletes them with the host, and shows or restores one by
 * handing the engine its blobs and the snapshot the checkpoint carries (`version.preview`,
 * `version.restore`), from which the engine reads the design it holds.
 */
import { useEffect, useState, useSyncExternalStore } from 'react'
import { deleteCheckpoint, historyErrorReason, listCheckpoints, readCheckpoint, updateCheckpoint, type KitCheckpoint } from '@kition/sdk/editor'
import { useDesignKit } from '../kit/hooks'
import { toBase64 } from '../menus/invoke'
import type { DesignKit } from '../kit/context'

export type { KitCheckpoint }

/** Labelled checkpoints asked per page (at most `CHECKPOINT_LIST_LIMIT`). */
export const CHECKPOINT_PAGE = 50

/** Whether the host keeps saved versions: checkpoints, served with the replica. */
export function hasCheckpoints(designKit: Pick<DesignKit, 'editor'>): boolean {
  const capabilities = designKit.editor.host.init.capabilities
  return capabilities.includes('checkpoints') && capabilities.includes('replica')
}

/** Whether the person may rename or delete `checkpoint`: only the kit's own (the host's are read-only). */
export const ownCheckpoint = (checkpoint: KitCheckpoint) => checkpoint.origin === 'kit'

const message = (error: unknown) => (error instanceof Error ? error.message : String(error))

/** A host refusal in words: its reason (`not-found`, `too-long`, `read-only`, `conflict`), else the message. */
export function refusal(t: DesignKit['t'], error: unknown): string {
  switch (historyErrorReason(error)) {
    case 'not-found':
      return t('This version is no longer kept')
    case 'too-long':
      return t('The title or description is too long')
    case 'read-only':
      return t('This file is read-only here')
    case 'conflict':
      return t('Someone changed this version meanwhile: try again')
    default:
      return message(error)
  }
}

let generation = 0
const listeners = new Set<() => void>()

/** The host's checkpoints changed here (saved, renamed, deleted): every list asks again. */
export function checkpointsChanged() {
  generation += 1
  for (const listener of listeners) listener()
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => {
    listeners.delete(listener)
  }
}

/** The document's saved versions (labelled checkpoints), newest first, `pages` pages of them. */
export function useCheckpoints(enabled: boolean) {
  const designKit = useDesignKit()
  const { host, session } = designKit.editor
  const available = hasCheckpoints(designKit)
  const changed = useSyncExternalStore(subscribe, () => generation)
  const [pages, setPages] = useState(1)
  const [state, setState] = useState<{ list: KitCheckpoint[]; more: boolean; error: string | null; loaded: boolean }>({ list: [], more: false, error: null, loaded: false })
  useEffect(() => {
    if (!enabled || !available) return undefined
    let current = true
    void (async () => {
      try {
        const list: KitCheckpoint[] = []
        let more = false
        let before: string | undefined
        for (let page = 0; page < pages; page += 1) {
          const answer = await listCheckpoints(host.files, session.path, { labelled: true, limit: CHECKPOINT_PAGE, ...(before ? { before } : {}) })
          if (!answer) break
          list.push(...answer.checkpoints)
          more = answer.more
          const last = answer.checkpoints[answer.checkpoints.length - 1]
          if (!more || !last) break
          before = last.id
        }
        if (current) setState({ list, more, error: null, loaded: true })
      } catch (error) {
        if (current) setState((now) => ({ ...now, error: refusal(designKit.t, error), loaded: true }))
      }
    })()
    return () => {
      current = false
    }
    // `designKit.t` changes with the locale only; the list follows the document and its changes.
  }, [enabled, available, changed, pages, host, session.path])
  return { ...state, available, older: () => setPages((now) => now + 1) }
}

/** A checkpoint as the host has it now, with the snapshot it carries (base64, as the engine's commands take it). */
export type LoadedCheckpoint = { checkpoint: KitCheckpoint; snapshot: string }

/**
 * Reads checkpoint `id` from the host and gives the engine the blobs its snapshot needs; answers
 * the checkpoint as the host has it now and its snapshot. Throws the host's refusal.
 */
export async function loadCheckpoint(designKit: DesignKit, id: string): Promise<LoadedCheckpoint> {
  const read = await readCheckpoint(designKit.editor.host.files, designKit.editor.session.path, id)
  if (!read) throw new Error(designKit.t('This host keeps no saved versions'))
  if (read.blobs.byteLength > 0) await designKit.invoke('addBlobs', new Uint8Array(read.blobs))
  return { checkpoint: read.checkpoint, snapshot: toBase64(new Uint8Array(read.snapshot)) }
}

/** Saves the design and records a saved version of it with the host (`saveCheckpoint`). */
export async function saveVersion(designKit: DesignKit, label: string, description: string): Promise<KitCheckpoint | null> {
  const saved = await designKit.editor.session.saveCheckpoint({ label, ...(description ? { description } : {}) })
  checkpointsChanged()
  return saved
}

/** Renames or redescribes one of the kit's saved versions (an empty description removes it). */
export async function renameVersion(designKit: DesignKit, checkpoint: KitCheckpoint, label: string, description: string) {
  const answer = await updateCheckpoint(designKit.editor.host.files, designKit.editor.session.path, checkpoint.id, { label, description })
  checkpointsChanged()
  return answer
}

/** Deletes one of the kit's saved versions, by its id. */
export async function deleteVersion(designKit: DesignKit, checkpoint: KitCheckpoint) {
  try {
    await deleteCheckpoint(designKit.editor.host.files, designKit.editor.session.path, checkpoint.id)
  } finally {
    checkpointsChanged()
  }
}

/** Shows saved version `checkpoint` on the canvas (read-only). */
export async function previewVersion(designKit: DesignKit, checkpoint: KitCheckpoint) {
  const { checkpoint: now, snapshot } = await loadCheckpoint(designKit, checkpoint.id)
  return designKit.run('version.preview', { snapshot, id: now.id, name: now.label ?? '', time: now.time })
}

/** Gives the design the content saved version `id` (a checkpoint's id) holds, as one undoable edit. */
export async function restoreVersion(designKit: DesignKit, id: string) {
  const { snapshot } = await loadCheckpoint(designKit, id)
  return designKit.run('version.restore', { snapshot })
}
