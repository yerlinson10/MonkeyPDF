import { copyFileSync, existsSync, mkdirSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { homedir } from 'node:os'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const crateDir = path.join(root, 'installer-webview')
const dest = path.join(root, 'src-tauri', 'windows', 'uninstall-ui.exe')

const cargoBin = path.join(homedir(), '.cargo', 'bin')
process.env.Path = `${cargoBin}${path.delimiter}${process.env.Path || ''}`

const built = spawnSync(
  'cargo',
  ['build', '--release', '--bin', 'monkeypdf-uninstaller'],
  { cwd: crateDir, stdio: 'inherit', env: process.env, shell: true },
)
if (built.status !== 0) process.exit(built.status ?? 1)

const exe = path.join(crateDir, 'target', 'release', 'monkeypdf-uninstaller.exe')
if (!existsSync(exe)) {
  console.error('No salió', exe)
  process.exit(1)
}
mkdirSync(path.dirname(dest), { recursive: true })
copyFileSync(exe, dest)
console.log('UI de desinstalación →', dest)
