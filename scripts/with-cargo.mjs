#!/usr/bin/env node
/**
 * Ensures ~/.cargo/bin is on PATH before running Tauri CLI.
 * Needed when the IDE terminal was opened before Rust was installed.
 */
import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { homedir } from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath } from 'node:url'

const cargoBin = path.join(homedir(), '.cargo', 'bin')
const pathKey = process.platform === 'win32' ? 'Path' : 'PATH'
const sep = path.delimiter
const current = process.env[pathKey] || process.env.PATH || ''

if (!current.split(sep).some((p) => path.resolve(p) === path.resolve(cargoBin))) {
  process.env[pathKey] = `${cargoBin}${sep}${current}`
  process.env.PATH = process.env[pathKey]
}

const args = process.argv.slice(2)
if (args.length === 0) {
  console.error('Usage: node scripts/with-cargo.mjs <command> [...args]')
  process.exit(1)
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const [cmd, ...cmdArgs] = args

// Prefer local Tauri CLI entrypoint to avoid shell PATH issues
let executable = cmd
let finalArgs = cmdArgs
if (cmd === 'tauri') {
  const tauriCli = path.join(root, 'node_modules', '@tauri-apps', 'cli', 'tauri.js')
  if (existsSync(tauriCli)) {
    executable = process.execPath
    finalArgs = [tauriCli, ...cmdArgs]
  }
}

const child = spawn(executable, finalArgs, {
  stdio: 'inherit',
  env: process.env,
  cwd: root,
})

child.on('exit', (code, signal) => {
  if (signal) {
    try {
      process.kill(process.pid, signal)
    } catch {
      /* ignore */
    }
  }
  process.exit(code ?? 1)
})
