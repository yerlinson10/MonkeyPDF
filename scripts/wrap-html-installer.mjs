import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { homedir } from 'node:os'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const nsisDir = path.join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis')
const crateDir = path.join(root, 'installer-webview')
const engineOut = path.join(crateDir, 'engine.exe')

const cargoBin = path.join(homedir(), '.cargo', 'bin')
process.env.Path = `${cargoBin}${path.delimiter}${process.env.Path || ''}`

if (!existsSync(nsisDir)) {
  console.error('No hay bundle NSIS. Corre antes: npm run tauri:build -- --bundles nsis')
  process.exit(1)
}

const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'))
const version = pkg.version
const setups = readdirSync(nsisDir).filter(
  (f) => f.endsWith('-setup.exe') && !f.includes('installer') && !f.includes('html') && !f.includes('new'),
)
const setup =
  setups.find((f) => f.includes(`_${version}_`)) ||
  setups.sort().at(-1)
if (!setup) {
  console.error('No encontré el setup NSIS en', nsisDir)
  process.exit(1)
}

copyFileSync(path.join(nsisDir, setup), engineOut)
const peek = readFileSync(engineOut)
if (!peek.includes(Buffer.from('Nullsoft'))) {
  console.error('setup.exe no es el motor NSIS (¿ya era el instalador HTML?). Corre npm run tauri:build:nsis')
  process.exit(1)
}

const built = spawnSync('cargo', ['build', '--release'], {
  cwd: crateDir,
  stdio: 'inherit',
  env: process.env,
  shell: true,
})
if (built.status !== 0) process.exit(built.status ?? 1)

const exeName = 'monkeypdf-installer.exe'
const builtExe = path.join(crateDir, 'target', 'release', exeName)
if (!existsSync(builtExe)) {
  console.error('No salió', builtExe)
  process.exit(1)
}

mkdirSync(nsisDir, { recursive: true })
for (const name of readdirSync(nsisDir)) {
  const full = path.join(nsisDir, name)
  if (name.endsWith('.exe') || name.endsWith('.WebView2') || name.includes('WebView2')) {
    rmSync(full, { recursive: true, force: true })
  }
}
const out = path.join(nsisDir, setup)
copyFileSync(builtExe, out)
console.log('Único instalador →', out)
