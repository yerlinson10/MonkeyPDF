# MonkeyPDF

Desktop PDF toolkit built with **Tauri 2**, **Svelte 5**, **Tailwind CSS**, and **Rust**.

Design system: see [`design.md`](design.md) + [`tokens.css`](tokens.css) (Hallmark workbench · banana stamp).

## Features

23 herramientas en el rail (más **Ajustes** en el pie). Orden = rail.

### Núcleo
- **Unir** — varios PDFs → uno, en el orden que elijas
- **Dividir** — extrae rangos de páginas a nuevos PDFs
- **Ordenar** — multi-archivo, grid arrastrable, rotar / borrar / insertar páginas
- **Rotar** — 90° / 180° / 270°, páginas concretas o documento; lote por carpeta
- **Comprimir** — recomprime imágenes o rasteriza a JPEG; lote por carpeta
- **PDF → JPG** — PDFium, DPI configurable
- **JPG → PDF** — JPG / PNG / WebP, ajuste proporcional
- **Extraer** — imágenes embebidas (XObject JPEG/PNG) o texto a TXT

### Suite
- **Proteger** — añadir o quitar contraseña de apertura (RC4 128-bit, lopdf)
- **Reparar** — diagnóstico + re-guardado limpio + reparación best-effort (xref, streams, huérfanos)
- **Metadatos** — leer/editar título, autor, asunto, palabras clave y fechas
- **Numerar** — sello de página (posición, formato, inicio, tamaño)
- **Office** — Word / Excel / PowerPoint / HTML / ODT / ODS / ODP ↔ PDF vía **LibreOffice** headless (`soffice` en PATH o Program Files)
- **PDF/A** — exporta A-1b / A-2b / A-3b (`SelectPdfVersion`)

### Avanzado
- **OCR** — [Tesseract](https://github.com/tesseract-ocr/tesseract) del sistema (`tesseract` en PATH o `C:\Program Files\Tesseract-OCR\`) → Markdown / TXT / PDF buscable; idiomas `spa` / `eng` / `spa+eng`
- **Censura** — negro permanente + flatten (sin texto ni campos copiables debajo)
- **Recorte** — CropBox + MediaBox
- **Marca de agua** — texto o imagen · posición 3×3 · mosaico · transparencia · rotación · capa; lote por carpeta
- **Comparar** — A|B, scroll sincronizado, informe de texto + heatmap visual, export `compare.md`
- **Firmar** — firma / iniciales / logo (escribir con fuentes cursivas, dibujar, subir imagen); arrastrar, mover, redimensionar; editar un activo actualiza todas las instancias; campos AcroForm (rellenar + «Firmar aquí»); sello de fecha; horneado visual (no es PKCS#7)
- **Editar** — canvas: texto existente (reemplazo o tapar+reescribir), añadir texto, anotar (resaltar / subrayar / tachar / nota), formas, mano alzada, imagen, sellos, borrado blanco, formularios; anotaciones PDF con appearance streams + **Aplanar**; undo/redo, zoom, páginas

### Markdown + IA
- **Markdown** — PDF → MD (heurísticas de títulos y tablas)
- **IA** — resumir o traducir con tu clave: OpenAI · Anthropic · **OpenRouter** · Ollama
- **Ajustes** — rutas de salida (carpeta por defecto + override por herramienta) y claves/modelos de IA, guardados en disco (`tauri-plugin-store`)

### Usabilidad (todas las herramientas)
- Arrastrar y soltar archivos; previsualización de páginas (zoom / pan)
- Progreso % + cancelar (OCR, PDF→JPG, Comprimir, Ordenar, Editar, lotes)
- Historial de recientes en la hoja de inicio (abrir en Explorer / repetir herramienta)
- Atajos: `Ctrl+K` busca · `Esc` cierra · `1–9` primeras tools · `Ctrl+O` abre archivo · `Ctrl+Enter` ejecuta
- Preferencias por herramienta (última calidad, ángulo, marca de agua…)
- Notificación nativa al terminar (clic abre Explorer en la salida)
- Menú contextual (copiar, pegar rutas, revelar en Explorer)
- Portapapeles del sistema (Tauri) para texto seleccionado en previews

## Prerequisites

- Node.js 20+
- Rust (stable) + MSVC Build Tools (Windows)
- `src-tauri/resources/pdfium.dll` (Windows x64 included)
- Optional: [LibreOffice](https://www.libreoffice.org/) for Office conversions
- Optional: [Tesseract OCR](https://github.com/UB-Mannheim/tesseract/wiki) with language packs **spa** and **eng** for OCR
- Optional: red for cursive signature fonts (Google Fonts: Great Vibes, Dancing Script, Sacramento, Allura)

## Develop

```bash
npm install
npm run tauri:dev
```

## Build

```bash
npm run tauri:build
```

Windows NSIS installer uses a forked template (`src-tauri/windows/installer.nsi`) with banana-stamp colors, custom buttons, and Spanish/English. Art: `npm run nsis:art`. First launch shows an in-app welcome sheet.

## Architecture

- Frontend: `src/` — Svelte workbench (N3 rail + sheet)
- Backend: `src-tauri/src/pdf_engine/` — lopdf · pdfium-render · LibreOffice · Tesseract · signatures · reqwest
