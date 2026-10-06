import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import { t } from './lib/i18n/t.js'

function showBootError(message) {
  const el = document.getElementById('grimoire-boot-error')
  if (!el) return
  el.style.display = 'block'
  el.textContent = message
}

window.addEventListener('error', (ev) => {
  showBootError(t('boot.scriptError', {
    message: ev.message,
    location: `${ev.filename ?? ''}:${ev.lineno ?? ''}`,
  }))
})

window.addEventListener('unhandledrejection', (ev) => {
  const r = ev.reason
  const msg =
    typeof r === 'string'
      ? r
      : r && typeof r === 'object' && 'message' in r
        ? String(r.message)
        : String(r)
  showBootError(t('boot.unhandled', { msg }))
})

let app
try {
  app = mount(App, {
    target: document.getElementById('app'),
  })
} catch (e) {
  showBootError(t('boot.mountFailed', { error: e }))
  throw e
}

document.getElementById('grimoire-boot-overlay')?.remove()

export default app
