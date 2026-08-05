<script lang="ts">
  import { save, open } from '@tauri-apps/plugin-dialog'
  import Icon from './Icon.svelte'

  interface Props {
    mode?: 'file' | 'directory'
    value?: string
    defaultName?: string
    filters?: Array<{ name: string; extensions: string[] }>
    label?: string
  }

  let {
    mode = 'file',
    value = $bindable(''),
    defaultName = 'output.pdf',
    filters = [{ name: 'PDF', extensions: ['pdf'] }],
    label = 'Archivo de salida',
  }: Props = $props()

  async function pick() {
    if (mode === 'directory') {
      const dir = await open({ directory: true, multiple: false })
      if (typeof dir === 'string') value = dir
      return
    }
    const path = await save({ defaultPath: defaultName, filters })
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
