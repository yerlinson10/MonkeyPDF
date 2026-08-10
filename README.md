# MonkeyPDF

Desktop PDF toolkit built with **Tauri 2**, **Svelte 5**, **Tailwind CSS**, and **Rust**.

Design system: see [`design.md`](design.md) + [`tokens.css`](tokens.css) (Hallmark workbench · banana stamp).

## Features

### Phase 1 — Núcleo
- Merge / Split / Rotate / Compress
- PDF → JPG (PDFium) · JPG/PNG/WebP → PDF

### Phase 2 — Suite
- Protect / Unlock (RC4 128-bit via lopdf)
- Page numbers
- Office ↔ PDF via **LibreOffice headless** (`soffice` on PATH or Program Files)

### Phase 3 — OCR, Censura, Recorte, Comparar
- **OCR** via system [Tesseract](https://github.com/tesseract-ocr/tesseract) (`tesseract` on PATH or `C:\Program Files\Tesseract-OCR\`) → Markdown / TXT / searchable PDF; langs `spa` / `eng` / `spa+eng`
- **Censura** — black burn + page flatten (no copyable text / form fields under redaction)
- **Recorte** — CropBox + MediaBox
- **Comparar** — side-by-side A|B, sync scroll, text report + visual heatmap, export `compare.md`

### Phase 4 — Markdown + IA
- PDF → Markdown (heuristics)
- Summarize / Translate with your API key (OpenAI · Anthropic · Ollama)
- Settings stored locally (`tauri-plugin-store`)

### Phase 5 — Firmar + formularios
- Editor de firma / iniciales / logo (escribir con fuentes cursivas, dibujar, subir imagen)
- Arrastrar al PDF, mover, redimensionar, eliminar; editar un activo actualiza todas las instancias
- Detección de campos AcroForm (rellenar + «Firmar aquí» en campos de firma)
- Horneado visual al PDF (no es firma criptográfica PKCS#7)

### Phase 6 — Reparar, Marca de agua, Ordenar
- **Reparar** — diagnóstico + re-guardado limpio + reparación best-effort (xref, streams, huérfanos)
- **Marca de agua** — texto o imagen · posición 3×3 · mosaico · transparencia · rotación · capa
- **Ordenar** — multi-archivo, grid de páginas arrastrable, rotar/borrar/insertar

### Phase 7 — Usabilidad
- Progreso % + cancelar en OCR, PDF→JPG, Comprimir, Ordenar
- Historial de recientes en la hoja de inicio
- Atajos: `Ctrl+K` busca · `Esc` cierra · `1–9` tools · `Ctrl+Enter` ejecuta
- Preferencias por herramienta (última calidad, ángulo, marca de agua…)
- **Metadatos** — leer/editar Info del PDF
- Lote por carpeta en Comprimir / Rotar / Marca de agua

### Phase 8 — PDF/A + Extraer
- **PDF/A** — exporta A-1b / A-2b / A-3b vía LibreOffice (`SelectPdfVersion`)
- **Extraer** — imágenes embebidas (XObject JPEG/PNG) o texto a TXT

### Phase 9 — Editar PDF (rail 21)
- Editor visual sobre el canvas: editar texto existente (reemplazo quirúrgico o tapar+reescribir), añadir texto, anotar (resaltar/subrayar/tachar/nota), formas, borrador a mano alzada, imagen, sellos, borrado blanco, formularios
- Anotaciones PDF reales con appearance streams + opción **Aplanar** al guardar
- Undo/redo, lista de ediciones, zoom/páginas, CTA «Guardar cambios»

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

## Architecture

- Frontend: `src/` — Svelte workbench (N3 rail + sheet)
- Backend: `src-tauri/src/pdf_engine/` — lopdf · pdfium-render · LibreOffice · Tesseract · signatures · reqwest
