/**
 * What the left panel package shows over the editor whether or not the panel is open: the Local
 * variables window (the rail's Variables button, the `variables` UI request), the variable picker
 * (`variablePicker.open`) and the style picker (`stylePicker.open`). The shell mounts it once.
 */
import { useEffect, useRef, useState } from 'react'
import { useUiRequest } from '../kit/requests'
import { StylePicker, type StylePickerRequest } from './StylePicker'
import { VariablePicker, type VariablePickerRequest } from './VariablePicker'
import { VariablesWindow } from './VariablesWindow'
import './left.css'

export function LeftHost() {
  const [variables, setVariables] = useState<{ collection: string | null } | null>(null)
  const [variablePicker, setVariablePicker] = useState<VariablePickerRequest | null>(null)
  const [stylePicker, setStylePicker] = useState<(StylePickerRequest & { key: number }) | null>(null)
  // The pointer's last press: a style picker opens at its height (upstream: the opener's y - 12).
  const lastY = useRef<number | undefined>(undefined)
  useEffect(() => {
    const down = (event: PointerEvent) => {
      lastY.current = event.clientY
    }
    window.addEventListener('pointerdown', down, true)
    return () => window.removeEventListener('pointerdown', down, true)
  }, [])
  useUiRequest('variables', (data) => {
    const collection = (data as { collection?: unknown } | null)?.collection
    setVariables({ collection: typeof collection === 'string' ? collection : null })
  })
  useUiRequest('variablePicker.open', (data) => setVariablePicker(data as VariablePickerRequest))
  useUiRequest('stylePicker.open', (data) => {
    const request = data as Partial<StylePickerRequest> | null
    if (!request?.slot) return
    setStylePicker({ ids: Array.isArray(request.ids) ? request.ids : [], slot: request.slot, y: request.y ?? lastY.current, edit: request.edit, key: Date.now() })
  })
  return (
    <>
      {variables ? <VariablesWindow initial={variables.collection} onClose={() => setVariables(null)} /> : null}
      {stylePicker ? <StylePicker key={stylePicker.key} request={stylePicker} onClose={() => setStylePicker(null)} /> : null}
      {variablePicker ? <VariablePicker request={variablePicker} onClose={() => setVariablePicker(null)} /> : null}
    </>
  )
}
