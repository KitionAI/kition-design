import { createRoot } from 'react-dom/client'
import { connectHost, createKitStrings, defineKitUi } from '@kition/sdk/editor'
import '@kition/sdk/editor/styles.css'
import './app.css'
import { App } from './App'
import { CATALOGS, loadStrings } from './strings'
import manifest from '../../kit.json'

/** Every element this editor draws besides engine commands, menus and tools (Kit UI protocol section 9). */
const KIT_UI = defineKitUi({
  panels: [
    { id: 'panel:layers', label: 'Layers' },
    { id: 'panel:properties', label: 'Design' },
  ],
  tabs: [],
  groups: [],
  controls: [],
})

/** The editor's own faces (Inter, upstream's UI font, and the CJK fallback), from the package's folder. */
function loadFonts() {
  const faces = [
    new FontFace('Inter', `url(${new URL('./fonts/Inter-Variable.ttf', document.baseURI).href})`, { weight: '100 900' }),
    new FontFace('Droid Sans Fallback', `url(${new URL('./fonts/DroidSansFallbackFull.ttf', document.baseURI).href})`),
  ]
  for (const face of faces) {
    document.fonts.add(face)
    void face.load().catch(() => undefined)
  }
}

loadFonts()
const root = createRoot(document.getElementById('root')!)

// Opened outside a host, the page says where it belongs instead of waiting forever.
const alone = window.setTimeout(() => {
  const t = createKitStrings(CATALOGS, navigator.language)
  root.render(<div className="kui-alone">{t('noHost')}</div>)
}, 3000)

// `replica`: engine contract 2. The `.kdesign` file carries the design's content; the host keeps its
// history (the replica) and relays changes between people editing one design (Kit UI protocol
// section 7). `checkpoints`: saved versions are the host's checkpoints of that history (section 7m).
void connectHost({ id: 'kit.design', version: manifest.version, capabilities: ['ai', 'replica', 'checkpoints'], brand: manifest.ui?.brand, display: manifest.ui?.display, ui: KIT_UI }).then(async (host) => {
  window.clearTimeout(alone)
  const show = async (locale: string) => {
    const strings = await loadStrings(locale)
    // No StrictMode: its doubled effects would open the document twice, in two engine workers.
    root.render(<App host={host} strings={strings} />)
  }
  await show(host.init.locale)
  // The editor follows the host's language.
  host.peer.on('ui/locale', ({ locale }) => void show(locale))
})
