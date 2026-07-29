import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'

function showBootError(message) {
  const el = document.getElementById('grimoire-boot-error')
  if (!el) return
  el.style.display = 'block'
  el.textContent = message
}

window.addEventListener('error', (ev) => {
  showBootError(`Script error: ${ev.message}\n${ev.filename ?? ''}:${ev.lineno ?? ''}`)
})

window.addEventListener('unhandledrejection', (ev) => {
  const r = ev.reason
  const msg =
    typeof r === 'string'
      ? r
      : r && typeof r === 'object' && 'message' in r
        ? String(r.message)
        : String(r)
  showBootError(`Unhandled: ${msg}`)
})

let app
try {
  app = mount(App, {
    target: document.getElementById('app'),
  })
} catch (e) {
  showBootError(`Mount failed: ${e}`)
  throw e
}

document.getElementById('grimoire-boot-overlay')?.remove()

export default app
