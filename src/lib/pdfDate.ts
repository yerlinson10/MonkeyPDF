/** PDF Info date helpers (`D:YYYYMMDDHHmmSS…`) ↔ ISO calendar day. */

const PDF_DATE_RE =
  /^D:(\d{4})(\d{2})(\d{2})(?:(\d{2})(\d{2})(\d{2}))?/i

export function parsePdfDate(raw: string): Date | null {
  const s = raw.trim()
  if (!s) return null
  const m = PDF_DATE_RE.exec(s)
  if (m) {
    const y = Number(m[1])
    const mo = Number(m[2]) - 1
    const d = Number(m[3])
    const hh = m[4] != null ? Number(m[4]) : 0
    const mm = m[5] != null ? Number(m[5]) : 0
    const ss = m[6] != null ? Number(m[6]) : 0
    const dt = new Date(y, mo, d, hh, mm, ss)
    return Number.isNaN(dt.getTime()) ? null : dt
  }
  // Already ISO or locale
  const iso = /^(\d{4})-(\d{2})-(\d{2})/.exec(s)
  if (iso) {
    const dt = new Date(Number(iso[1]), Number(iso[2]) - 1, Number(iso[3]))
    return Number.isNaN(dt.getTime()) ? null : dt
  }
  const loose = new Date(s)
  return Number.isNaN(loose.getTime()) ? null : loose
}

export function pdfDateToIso(raw: string): string {
  const d = parsePdfDate(raw)
  return d ? toIsoDay(d) : ''
}

export function isoToPdfDate(iso: string, timeFrom?: string): string {
  const day = iso.trim()
  if (!day) return ''
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day)
  if (!m) return ''
  let hh = '00'
  let mm = '00'
  let ss = '00'
  if (timeFrom) {
    const prev = PDF_DATE_RE.exec(timeFrom.trim())
    if (prev?.[4] != null) {
      hh = prev[4]
      mm = prev[5] ?? '00'
      ss = prev[6] ?? '00'
    }
  }
  return `D:${m[1]}${m[2]}${m[3]}${hh}${mm}${ss}`
}

export function toIsoDay(d: Date): string {
  const y = d.getFullYear()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${y}-${m}-${day}`
}

export function formatDisplayDate(
  isoOrDate: string | Date,
  formatId: string = 'es-short',
): string {
  const d =
    typeof isoOrDate === 'string'
      ? isoOrDate
        ? new Date(
            Number(isoOrDate.slice(0, 4)),
            Number(isoOrDate.slice(5, 7)) - 1,
            Number(isoOrDate.slice(8, 10)),
          )
        : null
      : isoOrDate
  if (!d || Number.isNaN(d.getTime())) return ''
  switch (formatId) {
    case 'es-long':
      return d.toLocaleDateString('es-ES', { day: 'numeric', month: 'long', year: 'numeric' })
    case 'iso':
      return toIsoDay(d)
    case 'us':
      return d.toLocaleDateString('en-US')
    case 'dot': {
      const dd = String(d.getDate()).padStart(2, '0')
      const mm = String(d.getMonth() + 1).padStart(2, '0')
      return `${dd}.${mm}.${d.getFullYear()}`
    }
    case 'es-short':
    default:
      return d.toLocaleDateString('es-ES')
  }
}

/** Heuristic: AcroForm field name looks like a date. */
export function looksLikeDateField(name: string): boolean {
  const n = name.toLowerCase().replace(/[\s_\-./]+/g, '')
  return (
    /fecha|date|fec|fch|nacimiento|birth|dob|datade|datasign|firmadate|signedon|day|dia/.test(
      n,
    ) && !/update|candidate|validate|invalidate/.test(n)
  )
}

export function isoFromLoose(value: string): string {
  if (!value.trim()) return ''
  const d = parsePdfDate(value) ?? (() => {
    const t = Date.parse(value)
    return Number.isNaN(t) ? null : new Date(t)
  })()
  return d ? toIsoDay(d) : ''
}
