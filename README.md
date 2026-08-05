# MonkeyPDF

Desktop PDF toolkit built with **Tauri 2**, **Svelte 5**, **Tailwind CSS**, and **Rust**.

## Phase 1 features

- Merge PDFs
- Split PDF by page ranges
- Rotate pages (90 / 180 / 270)
- Compress PDF (stream + image recompression)
- PDF → JPG (via PDFium)
- JPG/PNG/WebP → PDF

## Prerequisites

- Node.js 20+
- Rust (stable) + MSVC Build Tools (Windows)
- `src-tauri/resources/pdfium.dll` (already included for Windows x64)

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

- Frontend: `src/` (Svelte UI + Tauri `invoke` wrappers)
- Backend: `src-tauri/src/pdf_engine/` (lopdf + pdfium-render + image)
