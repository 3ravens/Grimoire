// Copyright (C) 2026 Wim Palland
//
// This file is part of Grimoire.
//
// Grimoire is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// Grimoire is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with Grimoire. If not, see <https://www.gnu.org/licenses/>.

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
