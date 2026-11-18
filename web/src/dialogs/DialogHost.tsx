/** Draws the open dialogs (`store.ts`), oldest first, so the newest is on top. */
import { BatchRenameDialog, FindReplaceDialog, LinkDialog, MoveToPageDialog, OffsetDialog } from './EditDialogs'
import { AboutDialog, ColorProfileDialog, ExportDialog, ImportReportDialog, MissingFontsDialog } from './FileDialogs'
import { PreferencesDialog } from './Preferences'
import { ShortcutsDialog } from './Shortcuts'
import { closeDialog, useDialogs, type OpenDialog } from './store'

function One({ dialog }: { dialog: OpenDialog }) {
  const close = () => closeDialog(dialog.kind)
  switch (dialog.kind) {
    case 'about':
      return <AboutDialog onClose={close} />
    case 'preferences':
      return <PreferencesDialog onClose={close} />
    case 'shortcuts':
      return <ShortcutsDialog onClose={close} />
    case 'findReplace':
      return <FindReplaceDialog onClose={close} />
    case 'batchRename':
      return <BatchRenameDialog ids={Array.isArray(dialog.data.ids) ? (dialog.data.ids as number[]) : []} onClose={close} />
    case 'link':
      return <LinkDialog onClose={close} />
    case 'offset':
      return <OffsetDialog onClose={close} />
    case 'moveToPage':
      return <MoveToPageDialog onClose={close} />
    case 'colorProfile':
      return <ColorProfileDialog onClose={close} />
    case 'missingFonts':
      return <MissingFontsDialog onClose={close} />
    case 'importReport':
      return <ImportReportDialog data={dialog.data} onClose={close} />
    case 'export':
      return <ExportDialog onClose={close} />
  }
}

export function DialogHost() {
  const dialogs = useDialogs()
  return (
    <>
      {dialogs.map((dialog) => (
        <One key={dialog.key} dialog={dialog} />
      ))}
    </>
  )
}
