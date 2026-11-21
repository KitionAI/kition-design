/**
 * The development host (dev.html): opens `public/sample/Sample.kdesign`, or a file chosen in the page.
 * Its in-memory workspace also holds a library file, `libraries/Design System.kdesign` (written by
 * `bindings/wasm/examples/sample_library.rs`), which the editor reads with `host/read` and
 * `host/stamp` like any workspace file; the link chooser (`links/pick`) answers it for "Add library file…".
 * Tests replace it in `window.devHost.files` (a new `version`) to play a library published again.
 */
import { startDevHost } from '@kition/sdk/dev-host'

startDevHost({
  sample: './sample/Sample.kdesign',
  files: { 'libraries/Design System.kdesign': './sample/libraries/Design System.kdesign' },
  pickable: /^libraries\/[^/]+\.kdesign$|\.(png|jpe?g|gif|svg|webp)$/i,
})
