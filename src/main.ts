import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { installDesktopShell } from './lib/desktop'

installDesktopShell()

const app = mount(App, {
  target: document.getElementById('app')!,
})

export default app
