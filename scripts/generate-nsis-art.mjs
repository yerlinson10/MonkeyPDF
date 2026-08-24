/**
 * Hallmark · component: nsis-installer · genre: playful-técnico · theme: press-shop banana stamp
 * states: default · hover · focus · active · disabled · loading · error · success
 * contrast: pass (NSIS Win32 chrome; branding is bitmap)
 *
 * MUI2 sizes: sidebar 164×314 · header 150×57 · 24-bit BMP (no alpha).
 */
import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import sharp from 'sharp'

const root = join(dirname(fileURLToPath(import.meta.url)), '..')
const outDir = join(root, 'src-tauri', 'windows')
const favicon = join(root, 'public', 'favicon.svg')
const iconPng = join(root, 'src-tauri', 'icons', 'icon.png')

const DESK = { r: 17, g: 26, b: 20 }
const PAPER = { r: 250, g: 248, b: 242 }

function encodeBmp24(width, height, rgb) {
  const stride = Math.ceil((width * 3) / 4) * 4
  const pixelSize = stride * height
  const header = 54
  const buf = Buffer.alloc(header + pixelSize)
  buf.write('BM', 0)
  buf.writeUInt32LE(buf.length, 2)
  buf.writeUInt32LE(header, 10)
  buf.writeUInt32LE(40, 14)
  buf.writeInt32LE(width, 18)
  buf.writeInt32LE(height, 22)
  buf.writeUInt16LE(1, 26)
  buf.writeUInt16LE(24, 28)
  buf.writeUInt32LE(pixelSize, 34)
  for (let y = 0; y < height; y++) {
    const srcY = height - 1 - y
    for (let x = 0; x < width; x++) {
      const i = (srcY * width + x) * 3
      const o = header + y * stride + x * 3
      buf[o] = rgb[i + 2]
      buf[o + 1] = rgb[i + 1]
      buf[o + 2] = rgb[i]
    }
  }
  return buf
}

async function toBmp(img, w, h, bg) {
  const png = await img
    .resize(w, h, { fit: 'fill' })
    .flatten({ background: bg })
    .png()
    .toBuffer()
  const { data } = await sharp(png).removeAlpha().raw().toBuffer({ resolveWithObject: true })
  return encodeBmp24(w, h, data)
}

function fillSvg(w, h, hex) {
  return Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">
      <rect width="100%" height="100%" fill="${hex}"/>
    </svg>`,
  )
}

async function sidebar() {
  const w = 164
  const h = 314
  const dots = []
  for (let y = 10; y < h; y += 14) {
    for (let x = 10; x < w; x += 14) {
      dots.push(`<circle cx="${x}" cy="${y}" r="1" fill="#2a3a30"/>`)
    }
  }

  const overlay = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
      <rect width="100%" height="100%" fill="#111A14"/>
      ${dots.join('')}
      <rect x="21" y="27" width="122" height="122" fill="#0E1612"/>
      <rect x="18" y="24" width="122" height="122" fill="#111A14" stroke="#FBB90A" stroke-width="2"/>
      <rect x="18" y="248" width="128" height="48" fill="#0E1612"/>
      <rect x="15" y="245" width="128" height="48" fill="#FAF8F2" stroke="#0E1612" stroke-width="2"/>
      <text x="27" y="266" font-family="Segoe UI, system-ui, sans-serif" font-size="13" font-weight="800" fill="#111A14">MONKEY</text>
      <text x="27" y="283" font-family="Segoe UI, system-ui, sans-serif" font-size="13" font-weight="800" fill="#FBB90A">PDF</text>
    </svg>`,
  )

  let monkey
  try {
    monkey = await sharp(await readFile(favicon))
      .resize(92, 92, { fit: 'contain', background: DESK })
      .png()
      .toBuffer()
  } catch {
    monkey = await sharp(iconPng).resize(92, 92).png().toBuffer()
  }

  const composed = await sharp(fillSvg(w, h, '#111A14'))
    .composite([
      { input: overlay, top: 0, left: 0 },
      { input: monkey, top: 38, left: 34 },
    ])
    .png()
    .toBuffer()

  return toBmp(sharp(composed), w, h, DESK)
}

async function header() {
  const w = 150
  const h = 57
  let monkey
  try {
    monkey = await sharp(await readFile(favicon))
      .resize(36, 36, { fit: 'contain', background: PAPER })
      .png()
      .toBuffer()
  } catch {
    monkey = await sharp(iconPng).resize(36, 36).png().toBuffer()
  }

  const overlay = Buffer.from(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}">
      <rect width="100%" height="100%" fill="#FAF8F2"/>
      <rect x="3" y="3" width="144" height="51" fill="none" stroke="#0E1612" stroke-width="2"/>
      <rect x="10" y="10" width="38" height="38" fill="#111A14" stroke="#FBB90A" stroke-width="2"/>
      <text x="56" y="28" font-family="Segoe UI, system-ui, sans-serif" font-size="11" font-weight="800" fill="#111A14">MONKEY</text>
      <text x="56" y="44" font-family="Segoe UI, system-ui, sans-serif" font-size="11" font-weight="800" fill="#C78D0F">HAZLO LOCAL</text>
    </svg>`,
  )

  const composed = await sharp(fillSvg(w, h, '#FAF8F2'))
    .composite([
      { input: overlay, top: 0, left: 0 },
      { input: monkey, top: 12, left: 12 },
    ])
    .png()
    .toBuffer()

  return toBmp(sharp(composed), w, h, PAPER)
}

await mkdir(outDir, { recursive: true })
const sideBmp = await sidebar()
const headBmp = await header()
await writeFile(join(outDir, 'installer-sidebar.bmp'), sideBmp)
await writeFile(join(outDir, 'installer-header.bmp'), headBmp)
console.log('NSIS art → src-tauri/windows/installer-sidebar.bmp + installer-header.bmp')
