// Electron preload — bridges main → renderer with contextIsolation on.
//
// Only one piece of state crosses the bridge: the localhost URL of
// the Rust backend. The main process discovered the kernel-assigned
// port at startup and passed it via `webPreferences.additionalArguments`
// as `--ani-gui-api-base=http://127.0.0.1:<port>`. We parse it here
// and surface it as `window.aniGui.apiBase`.
//
// The frontend's `lib/api.ts > apiBase()` checks `window.aniGui.apiBase`
// first; once this value is present, all `fetch()` calls land on the
// Rust sidecar.

'use strict';

const { contextBridge, ipcRenderer } = require('electron');

const flag = '--ani-gui-api-base=';
const arg = process.argv.find((a) => a.startsWith(flag));
const apiBase = arg ? arg.slice(flag.length) : null;

if (!apiBase) {
	console.error('[preload] no --ani-gui-api-base flag — renderer will fail to reach the backend');
}

// Capture the locale main.js read from config.toml so the renderer
// can flip Paraglide directly at boot (see src/hooks.client.ts).
// Previous design relayed through localStorage and raced Paraglide's
// bootstrap; the renderer now reads `window.aniGui.configLocale`
// from the contextBridge below and calls setLocale before any
// `m.foo()` evaluates. Missing flag → null → renderer falls back to
// Paraglide's own resolution.
const localeFlag = '--ani-gui-locale=';
const localeArg = process.argv.find((a) => a.startsWith(localeFlag));
const configLocale = localeArg ? localeArg.slice(localeFlag.length) : null;

// Per-process random secret printed by the backend at startup and
// passed through additionalArguments. The renderer attaches it as the
// `x-ani-gui-internal-secret` header on the few backend paths that
// need a renderer-only gate (the disconnect-after-expiry cache wipe,
// Codex P2 #3370011855). A cross-origin tab under the permissive CORS
// layer can't guess 32 bytes of entropy, so the attack closes.
const internalSecretFlag = '--ani-gui-internal-secret=';
const internalSecretArg = process.argv.find((a) => a.startsWith(internalSecretFlag));
const internalSecret = internalSecretArg
	? internalSecretArg.slice(internalSecretFlag.length)
	: null;

contextBridge.exposeInMainWorld('aniGui', {
	apiBase,
	internalSecret,

	// Surface `process.platform` so renderer-side UI can tailor copy
	// per OS (Windows installer vs Linux package manager vs macOS
	// Homebrew). Values match Node's `process.platform`: 'win32',
	// 'linux', 'darwin', 'freebsd', etc.
	platform: process.platform,

	// Native folder picker — opens an OS dialog, returns the chosen
	// absolute path or `null` on cancel. The renderer's download
	// confirmation modal calls this when the user clicks "Browse…".
	async pickDirectory(options) {
		return ipcRenderer.invoke('ani-gui:pick-directory', options || {});
	},

	// Native file picker — same shape as pickDirectory but for a
	// single file. The settings page uses this to let the user point
	// at an external-player executable that isn't on PATH. `options`
	// supports `{ title, defaultPath, filters }` where `filters` is
	// the Electron OpenDialog filter shape:
	//   [{ name: "Executables", extensions: ["exe"] }]
	async pickFile(options) {
		return ipcRenderer.invoke('ani-gui:pick-file', options || {});
	},

	// Open the OS file manager at the given directory. Used by the
	// download dock and the completion toast's "Reveal in folder"
	// button. Returns `true` on success, `false` if the path didn't
	// resolve.
	async revealInFolder(dirPath) {
		return ipcRenderer.invoke('ani-gui:reveal-in-folder', dirPath);
	},

	// Read the CURRENT locale from config.toml synchronously. Re-reads
	// the file every call (main.js handler) so a renderer reload
	// after a Settings change sees the just-persisted value — the
	// static `configLocale` snapshot taken at window-creation time is
	// frozen and would silently revert the user's pick on reload.
	// Returns null when no config or no `locale` key. Used at boot
	// by `src/hooks.client.ts`.
	getConfigLocale() {
		return ipcRenderer.sendSync('ani-gui:read-config-locale');
	},

	// Push the renderer's current active-download count to main so
	// the close handler can decide whether to prompt the user before
	// quitting. Fire-and-forget — main caches the latest value and
	// reads it synchronously at close time. Called from the layout's
	// download-store effect; no return value needed.
	notifyActiveDownloads(count) {
		ipcRenderer.send('ani-gui:active-downloads', count);
	},

	// Open an https URL in the OS browser. Used by the /account
	// page's Privacy Policy link. Main's handler rejects non-http(s)
	// schemes so a compromised renderer can't redirect to file://.
	openExternal(url) {
		return ipcRenderer.invoke('ani-gui:open-external', url);
	},

	// Window controls for the custom frameless titlebar. The renderer
	// draws its own minimize/maximize/close (Electron is frameless so the
	// OS doesn't), and these relay the intent to the main process.
	// `isMaximized()` is sync so the button paints the right icon on first
	// render; `onMaximizeChange` keeps it live as the WM maximizes/restores
	// out from under us. Returns an unsubscribe fn.
	windowControls: {
		minimize() {
			ipcRenderer.send('ani-gui:window:minimize');
		},
		toggleMaximize() {
			ipcRenderer.send('ani-gui:window:toggle-maximize');
		},
		close() {
			ipcRenderer.send('ani-gui:window:close');
		},
		isMaximized() {
			return ipcRenderer.sendSync('ani-gui:window:is-maximized');
		},
		onMaximizeChange(cb) {
			const listener = (_e, isMax) => cb(Boolean(isMax));
			ipcRenderer.on('ani-gui:window:maximize-changed', listener);
			return () => ipcRenderer.removeListener('ani-gui:window:maximize-changed', listener);
		}
	},

	// Account integration surface. Mirrors the lifecycle documented in
	// .planning/account-integration.md §3.3 / §3.4:
	//
	//   1. account.openOAuth({authUrl}) → opens browser, waits for
	//      callback, resolves to {ok:true, code, state} or
	//      {ok:false, kind:'port_busy'|'timeout'|'cancelled'|...}.
	//   2. account.setToken(provider, payload) → safeStorage-encrypts
	//      the payload (`{access_token, refresh_token, expires_at_epoch_s,
	//      user_id}`) and writes to disk.
	//   3. account.getToken(provider) → sync read + decrypt; returns
	//      {ok:true, payload} or {ok:false, kind:'not_found'|...}.
	//   4. account.clearToken(provider) → drops the persisted file.
	//
	// Sync `getToken` because every backend call that needs a bearer
	// reads it inline; an async round-trip per fetch is unnecessary
	// IPC overhead.
	account: {
		openOAuth(args) {
			return ipcRenderer.invoke('ani-gui:account:open-oauth', args);
		},
		cancelOAuth() {
			return ipcRenderer.invoke('ani-gui:account:cancel-oauth');
		},
		setToken(provider, payload) {
			return ipcRenderer.invoke('ani-gui:account:set-token', { provider, payload });
		},
		getToken(provider) {
			return ipcRenderer.sendSync('ani-gui:account:get-token', provider);
		},
		clearToken(provider) {
			return ipcRenderer.invoke('ani-gui:account:clear-token', provider);
		}
	}
});
