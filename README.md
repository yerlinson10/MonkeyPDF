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
- [Lista de funciones](#lista-de-funciones)

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

---

## Lista de funciones

Las **23** del rail, con todas las funciones del sistema. **Ajustes** está en el pie.

### 01 · Unir PDF

- Combina 2 o más PDFs en un archivo.
- Lista ordenable (subir / bajar).
- Salida: un PDF.

### 02 · Dividir PDF

- Un PDF de entrada; muestra el recuento de páginas.
- Rangos `1-3, 5, 7-9` (coma; rango o página suelta).
- Un PDF por rango en la carpeta de salida.

### 03 · Ordenar

- Uno o más PDFs; miniaturas en grid arrastrable.
- Por página: rotar 0° / 90° / 180° / 270°, eliminar.
- Insertar otro PDF entre páginas o al final.
- Reordenar archivos de origen; restablecer.
- Un PDF de salida.

### 04 · Rotar PDF

- Modo archivo o carpeta (lote).
- Ángulo 90° / 180° / 270°.
- Páginas opcionales (`1,3,5`); vacío = todas.
- Lote: carpeta `_rot{ángulo}`.

### 05 · Comprimir PDF

- Modo archivo o carpeta (lote).
- Calidad JPEG 10–95 (paso 5; por defecto 70).
- Recomprime imágenes embebidas o rasteriza páginas a JPEG.

### 06 · PDF a JPG

- Un PDF; una JPG por página.
- DPI 72–300 (atajos 72 / 150 / 300).
- Render PDFium.

### 07 · JPG a PDF

- JPG / JPEG / PNG / WebP, lista ordenable.
- Una página por imagen, ajuste a página completa.

### 08 · Extraer

- **Imágenes:** XObjects JPEG/PNG (no rasteriza) → carpeta.
- **Texto:** plano a TXT.

### 09 · Proteger

- Modo **Proteger** o **Desbloquear**.
- Contraseña de usuario; en proteger, contraseña de propietario opcional.
- Cifrado RC4 128-bit (apertura).

### 10 · Reparar

- Diagnóstico: versión, páginas, cifrado, EOF, streams, huérfanos, xref, linearizado.
- Contraseña si el PDF está cifrado.
- Re-guardado limpio y reparación best-effort; rescate parcial posible.

### 11 · Metadatos

- Lee y edita: título, autor, asunto, palabras clave, creador, productor.
- Fechas de creación y modificación.
- Muestra el número de páginas. Guarda un PDF nuevo.

### 12 · Numerar

- Posición: abajo centro / derecha / izquierda, arriba centro.
- Formato con `{n}` y `{total}` (por defecto `{n} / {total}`).
- Empezar en (≥ 1); tamaño 8–24 pt.

### 13 · Office

- Requiere LibreOffice (`soffice`).
- Entrada: PDF, DOC/DOCX, XLS/XLSX, PPT/PPTX, HTML, ODT/ODS/ODP.
- Destino: PDF, DOCX, XLSX, PPTX o HTML.

### 14 · PDF/A

- Requiere LibreOffice. Un PDF.
- Niveles: **PDF/A-1b** (clásico), **A-2b** (recomendado, PDF 1.7), **A-3b** (adjuntos).

### 15 · OCR

- Tesseract del sistema; idiomas `spa+eng` / `spa` / `eng`.
- Salida: Markdown, TXT o PDF buscable.

### 16 · Censura

- Negro permanente: aplana la página (sin texto ni campos debajo).
- Varias zonas, por página; zoom; limpiar página; quitar zona.

### 17 · Recorte

- Rectángulo sobre la página; CropBox + MediaBox.
- Opción de aplicar el mismo recorte a todas las páginas.

### 18 · Marca de agua

- Archivo o carpeta (lote); vista previa con zoom.
- Texto (CONFIDENCIAL, B/I/U, color, tamaño 8–120) o imagen (PNG/JPG/WebP, escala 5–90%).
- Rejilla 3×3; mosaico; transparencia 0–100%; rotación 0–360°.
- Capa encima o debajo; rango de páginas desde–hasta.

### 19 · Comparar

- PDF A (original) + PDF B (nuevo).
- Modo texto + visual, solo texto o solo visual.
- Lado a lado o mapa visual; scroll sincronizado.
- Informe (solo en A/B, distinto, px cambiados); saltar a un cambio; exportar `compare.md`.

### 20 · Firmar

- Firma, iniciales o logo: escribir (Great Vibes, Dancing Script, Sacramento, Allura), dibujar o subir imagen.
- Arrastrar, mover y redimensionar; editar un activo actualiza todas las instancias.
- Campos Nombre / Fecha / Texto (formatos D/M/AAAA, largo, ISO, US, DD.MM).
- AcroForm: rellenar, «Firmar aquí», checkbox, select, fecha.
- Horneado visual (no es PKCS#7 / certificado digital).

### 21 · Editar

- Modos: editar texto, añadir texto, anotar, formas, dibujar, imagen, sello, borrar blanco, formulario.
- Anotar: resaltar / subrayar / tachar / nota.
- Formas: rectángulo, elipse, línea (relleno, trazo, grosor, opacidad).
- Texto: Helvetica / Times / Courier; tamaño; B/I; alineación; color; selección de palabras.
- Sellos: aprobado, rechazado, confidencial, borrador, firmado, urgente, copia, original, texto propio.
- Formulario: campos de texto, choice, firma.
- Undo/redo, zoom, lista de cambios, limpiar página, **Aplanar**, guardar.

### 22 · Markdown

- PDF con texto nativo → `.md` (heurísticas de títulos y tablas).
- Escaneos: usar OCR primero.

### 23 · IA

- Un PDF. Proveedor y modelo en Ajustes (OpenAI, Anthropic, OpenRouter, Ollama).
- **Resumir** o **Traducir** (idioma destino, por defecto español).
- Vista previa y guardado opcional `.md` / `.txt`.

### Ajustes

- **Ajustes** — Rutas de salida (carpeta por defecto + override por herramienta) y claves/modelos de IA, guardados en disco (`tauri-plugin-store`).

### Usabilidad (todas las herramientas)

- Arrastrar y soltar archivos; previsualización de páginas (zoom / pan).
- Progreso % + cancelar (OCR, PDF→JPG, Comprimir, Ordenar, Editar, lotes).
- Historial de recientes (abrir en Explorer / repetir herramienta).
- Atajos: `Ctrl+K` busca · `Esc` cierra · `1–9` primeras tools · `Ctrl+O` abre archivo · `Ctrl+Enter` ejecuta.
- Preferencias por herramienta (última calidad, ángulo, marca de agua…).
- Notificación nativa al terminar (clic abre Explorer en la salida).
- Menú contextual (copiar, pegar rutas, revelar en Explorer).
- Portapapeles del sistema para texto seleccionado en previews.
