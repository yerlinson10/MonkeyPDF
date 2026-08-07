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
