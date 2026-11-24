import { readFileSync } from 'node:fs'

import { describe, expect, it } from 'vitest'

const componentSource = readFileSync(
  new URL('./components/QuickView.vue', import.meta.url),
  'utf8',
)
const windowSource = readFileSync(
  new URL('../src-tauri/src/runtime/quick_view_window.rs', import.meta.url),
  'utf8',
)

describe('quick view window frame', () => {
  it('uses one rounded frame without a clipped or rectangular outer shadow', () => {
    expect(componentSource).toMatch(
      /\.quick-shell\s*\{[^}]*padding:\s*1px;/s,
    )
    expect(componentSource).toMatch(
      /\.quick-panel\s*\{[^}]*border:\s*1px\s+solid[^;}]*;[^}]*box-shadow:\s*inset\s+0\s+1px/s,
    )
    expect(componentSource).not.toMatch(
      /\.quick-panel\s*\{[^}]*box-shadow:[^;}]*0\s+22px\s+54px/s,
    )
    expect(windowSource).toMatch(/\.shadow\(false\)/)
  })
})
