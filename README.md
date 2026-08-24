# MonkeyPDF

Suite de PDF de escritorio. **Local, sin cuenta y sin subir archivos a la nube.**

Windows · Tauri 2 · Svelte 5 · Rust · `v0.1.1`

Unir, dividir, comprimir, firmar, OCR, Office y 23 herramientas en un workbench. El PDF no sale de tu máquina; las claves de IA (opcional) las pones tú.

---

## Contenido

- [Descargar](#descargar)
- [Herramientas](#herramientas)
- [Requisitos opcionales](#requisitos-opcionales)
- [Desarrollo](#desarrollo)
- [Compilar](#compilar)
- [Arquitectura](#arquitectura)
- [Privacidad](#privacidad)

---

## Descargar

Instalador NSIS para Windows x64 (usuario actual, sin privilegios de administrador):

```text
src-tauri/target/release/bundle/nsis/
```

Tras `npm run tauri:build:nsis` el artefacto listo para distribuir es el instalador envuelto (HTML + motor WebView).

Primera ejecución: hoja de bienvenida en la app.

---

## Herramientas

23 herramientas en el rail + **Ajustes** en el pie. Arrastrar y soltar, vista previa con zoom/pan, progreso y cancelar, recientes, atajos (`Ctrl+K`, `Ctrl+O`, `Ctrl+Enter`, `Esc`, `1–9`).

| Grupo | Herramientas |
| --- | --- |
| **Núcleo** | Unir · Dividir · Ordenar · Rotar · Comprimir · PDF → JPG · JPG → PDF · Extraer (imágenes / texto) |
| **Suite** | Proteger (contraseña RC4) · Reparar · Metadatos · Numerar · Office (LibreOffice) · PDF/A |
| **Avanzado** | OCR (Tesseract) · Censura permanente · Recorte · Marca de agua · Comparar · Firmar · Editar |
| **Markdown + IA** | PDF → Markdown · Resumir / traducir (OpenAI, Anthropic, OpenRouter, Ollama) |

Notas rápidas:

- **Firmar** es sello visual (texto, dibujo, imagen, AcroForm). No es PKCS#7 / certificado digital.
- **Office** y **PDF/A** usan LibreOffice headless (`soffice`).
- **OCR** usa Tesseract del sistema (`spa` / `eng`).
- Lote por carpeta en rotar, comprimir y marca de agua.

---

## Requisitos opcionales

| Para | Qué instalar |
| --- | --- |
| Word / Excel / PowerPoint / HTML / ODT ↔ PDF y PDF/A | [LibreOffice](https://www.libreoffice.org/) (`soffice` en PATH o Program Files) |
| OCR | [Tesseract](https://github.com/UB-Mannheim/tesseract/wiki) con paquetes **spa** y **eng** |
| Firmas cursivas | Las fuentes se descargan de Google Fonts (Great Vibes, Dancing Script, Sacramento, Allura) |

El núcleo del PDF (unir, dividir, comprimir, editar, etc.) no necesita nada extra. PDFium para Windows x64 va en `src-tauri/resources/pdfium.dll`.

---

## Desarrollo

### Requisitos

- Node.js 20+
- Rust stable
- Windows: [MSVC Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

```bash
npm install
npm run tauri:dev
```

### Scripts

| Comando | Qué hace |
| --- | --- |
| `npm run tauri:dev` | App en modo desarrollo |
| `npm run tauri:build` | Bundle Tauri (NSIS + recursos) |
| `npm run tauri:build:nsis` | NSIS + desinstalador + envoltorio HTML del instalador |
| `npm run nsis:art` | Regenera arte del instalador |
| `npm run check` | Typecheck Svelte / TypeScript |

El instalador NSIS usa plantilla propia (`src-tauri/windows/installer.nsi`), idiomas ES/EN y paleta banana-stamp.

---

## Compilar

```bash
npm install
npm run tauri:build:nsis
```

Salida típica: `src-tauri/target/release/bundle/nsis/` y el instalador envuelto que genera `wrap:installer`.

---

## Arquitectura

```text
src/                      UI Svelte (rail + hoja)
src-tauri/src/pdf_engine/ Motor Rust (lopdf, pdfium-render, qpdf, LibreOffice, Tesseract, firmas)
src-tauri/windows/        Plantilla NSIS e iconos
installer-webview/        Envoltorio visual del instalador
```

Ajustes (rutas de salida, claves y modelos de IA) se guardan en disco con `tauri-plugin-store`.

Diseño interno: [`design.md`](design.md) y [`tokens.css`](tokens.css).

---

## Privacidad

- Los PDF se procesan **en local**.
- La IA es opt-in: la petición sale a la API que configures (o a Ollama en tu red).
- Sin telemetría de producto en este repositorio.

---

## Licencia

Aún no hay archivo `LICENSE` en el repo. Añádelo antes de publicar (MIT, Apache-2.0, GPL, etc.) y enlázalo aquí.
