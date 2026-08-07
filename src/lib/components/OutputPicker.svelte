<script lang="ts">
  import { onMount } from 'svelte'
  import { save, open } from '@tauri-apps/plugin-dialog'
  import Icon from './Icon.svelte'
  import { joinOutputPath, resolveOutputDir, type OutputToolId } from '../settings'

  interface Props {
    mode?: 'file' | 'directory'
    value?: string
    defaultName?: string
    filters?: Array<{ name: string; extensions: string[] }>
    label?: string
    /** Herramienta: aplica la ruta por defecto / avanzada de Ajustes. */
    tool?: OutputToolId
    /** Si true (default), rellena la ruta al cargar cuando hay carpeta configurada. */
    autofill?: boolean
  }

  let {
    mode = 'file',
    value = $bindable(''),
    defaultName = 'output.pdf',
    filters = [{ name: 'PDF', extensions: ['pdf'] }],
    label = 'Archivo de salida',
    tool,
    autofill = true,
  }: Props = $props()

  let preferredDir = $state('')

  onMount(() => {
    void applyPreferredPath()
  })

  async function applyPreferredPath() {
    preferredDir = await resolveOutputDir(tool)
    if (!autofill || !preferredDir || value) return
    value = mode === 'directory' ? preferredDir : joinOutputPath(preferredDir, defaultName)
  }

  async function pick() {
    const startDir = preferredDir || (await resolveOutputDir(tool))
    preferredDir = startDir

    if (mode === 'directory') {
      const dir = await open({
        directory: true,
        multiple: false,
        defaultPath: startDir || undefined,
      })
      if (typeof dir === 'string') value = dir
      return
    }

    const suggested = startDir ? joinOutputPath(startDir, defaultName) : defaultName
    const path = await save({ defaultPath: suggested, filters })
    if (path) value = path
  }
</script>

<div class="mp-field">
  <label for="output-path">{label}</label>
  <div class="flex gap-2">
    <input
      id="output-path"
      class="mp-input mono"
      bind:value
      placeholder={mode === 'directory' ? 'Selecciona una carpeta…' : 'Selecciona ruta de salida…'}
      readonly
    />
    <button type="button" class="mp-btn mp-btn-ghost shrink-0" onclick={pick}>
      <Icon name="folder" size={16} />
      Examinar
    </button>
  </div>
</div>
