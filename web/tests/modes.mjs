// The DesignKit editor in each display mode (kit-editor's EditorFrame: Menus, Single line, Classic, and
// Focus), through the development host: node tests/modes.mjs [chromium|webkit]
// kition-sdk's mode smoke (`@kition/sdk/dev-host/mode-smoke`). Run it with no HTTP proxy set.
// Single line opens on Edit: cut, copy, paste, duplicate, delete, find and replace, select all.
import { chromium, webkit } from 'playwright'
import { createServer } from 'vite'
import { runModeSmoke } from '@kition/sdk/dev-host/mode-smoke'

const EDIT_ROW = ['edit.cut', 'edit.copy', 'edit.paste', 'edit.duplicate', 'edit.delete', 'edit.findReplace', 'edit.selectAll']
const line = { tab: 'Edit', primary: EDIT_ROW.map((command) => `design-cmd-${command}`), min: 5 }
await runModeSmoke({ kit: 'kit.design', line, config: new URL('../vite.config.ts', import.meta.url).pathname, browser: process.argv[2] === 'webkit' ? webkit : chromium, createServer })
