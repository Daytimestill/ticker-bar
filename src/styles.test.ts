import { readFileSync } from 'node:fs'

import { describe, expect, it } from 'vitest'

const stylesheet = readFileSync(new URL('./styles.css', import.meta.url), 'utf8')

describe('settings layout wrapping rules', () => {
  it('keeps compact control labels on one line', () => {
    expect(stylesheet).toMatch(
      /\.version,[\s\S]*\.primary[\s\S]*?\{[\s\S]*?white-space:\s*nowrap/,
    )
  })

  it('keeps the menu bar preview on one line', () => {
    expect(stylesheet).toMatch(
      /\.preview-card strong\s*\{[^}]*white-space:\s*nowrap/s,
    )
  })

  it('reduces dense option rows to two columns before labels get squeezed', () => {
    expect(stylesheet).toMatch(
      /@media \(max-width:\s*700px\)[\s\S]*?\.item-options\s*\{[^}]*grid-template-columns:\s*repeat\(2,/,
    )
  })

  it('keeps each runtime switch next to its explanatory text', () => {
    expect(stylesheet).toMatch(
      /\.runtime-option\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*auto\s+minmax\(0,\s*1fr\);[^}]*gap:\s*12px;/s,
    )
  })

  it('uses a two-pane application shell and an asymmetric display workbench', () => {
    expect(stylesheet).toMatch(
      /\.settings-layout\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*204px\s+minmax\(0,\s*1fr\);/s,
    )
    expect(stylesheet).toMatch(
      /\.display-workbench\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(0,\s*1\.6fr\)\s+minmax\(260px,\s*1fr\);/s,
    )
  })

  it('uses the approved graphite visual system instead of light cards', () => {
    expect(stylesheet).toMatch(/--canvas:\s*#111217;/)
    expect(stylesheet).toMatch(/--surface:\s*rgb\(255 255 255 \/ 4%\);/)
    expect(stylesheet).toMatch(/--accent:\s*#7c73f1;/)
    expect(stylesheet).toMatch(
      /\.settings-shell\s*\{[^}]*background:[^;}]*radial-gradient[^;}]*linear-gradient/s,
    )
    expect(stylesheet).toMatch(
      /\.workspace-header\s*\{[^}]*backdrop-filter:\s*blur\(24px\)\s+saturate\(130%\);/s,
    )
  })

  it('gives the stock page a master list and a dedicated inspector', () => {
    expect(stylesheet).toMatch(
      /\.stock-workbench\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(0,\s*1\.45fr\)\s+minmax\(300px,\s*0\.9fr\);/s,
    )
    expect(stylesheet).toMatch(
      /\.stock-inspector-pane\s*\{[^}]*border-left:\s*1px\s+solid\s+var\(--line\);/s,
    )
  })

  it('uses workspace container queries at the real minimum window width', () => {
    expect(stylesheet).toMatch(
      /\.settings-workspace\s*\{[^}]*container-type:\s*inline-size;[^}]*container-name:\s*settings-workspace;/s,
    )
    expect(stylesheet).toMatch(
      /@container settings-workspace \(max-width:\s*720px\)[\s\S]*?\.stock-workbench\s*\{[^}]*grid-template-columns:\s*1fr;/,
    )
    expect(stylesheet).toMatch(
      /@container settings-workspace \(max-width:\s*720px\)[\s\S]*?\.display-workbench\s*\{[^}]*grid-template-columns:\s*1fr;/,
    )
    expect(stylesheet).toMatch(
      /@container settings-workspace \(max-width:\s*620px\)[\s\S]*?\.alert-row\s*\{[^}]*grid-template-columns:\s*auto\s+minmax\(0,\s*1fr\);/,
    )
  })

  it('keeps every stock row on the same six-column track', () => {
    expect(stylesheet).toMatch(
      /\.stock-row\s*\{[^}]*grid-template-columns:\s*24px\s+minmax\(90px,\s*1fr\)\s+46px\s+52px\s+56px\s+42px;/s,
    )
    expect(stylesheet).not.toMatch(/\.stock-row-actions:empty\s*\{[^}]*display:\s*none;/s)
  })

  it('balances portfolio totals as a full-width metric grid', () => {
    expect(stylesheet).toMatch(
      /\.position-total-item\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(100px,\s*0\.8fr\)\s+repeat\(4,\s*minmax\(100px,\s*1fr\)\);/s,
    )
    expect(stylesheet).toMatch(
      /\.position-total-metric\s*\{[^}]*min-width:\s*0;[^}]*border-left:\s*1px\s+solid\s+var\(--line\);/s,
    )
  })

  it('uses one readable type and control-size system', () => {
    expect(stylesheet).toMatch(/--font-body:\s*13px;/)
    expect(stylesheet).toMatch(/--control-height:\s*36px;/)
    expect(stylesheet).toMatch(/--control-height-compact:\s*30px;/)
    expect(stylesheet).toMatch(
      /select\s*\{[^}]*appearance:\s*none;[^}]*background-image:\s*url\(/s,
    )
  })

  it('uses a compact density scale without shrinking the body text', () => {
    expect(stylesheet).toMatch(
      /\.workspace-header\s*\{[^}]*height:\s*88px;[^}]*padding:\s*18px\s+24px\s+14px;/s,
    )
    expect(stylesheet).toMatch(
      /\.workspace-scroll\s*\{[^}]*height:\s*calc\(100vh\s*-\s*88px\);[^}]*padding:\s*12px\s+24px\s+24px;/s,
    )
    expect(stylesheet).toMatch(
      /\.preview-card\s*\{[^}]*min-height:\s*48px;/s,
    )
    expect(stylesheet).toMatch(
      /\.stock-row\s*\{[^}]*min-height:\s*58px;/s,
    )
    expect(stylesheet).toMatch(
      /\.item-summary\s*\{[^}]*min-height:\s*52px;/s,
    )
    expect(stylesheet).toMatch(
      /\.runtime-option\s*\{[^}]*min-height:\s*54px;/s,
    )
    expect(stylesheet).toMatch(
      /\.position-row > label\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*54px\s+minmax\(0,\s*1fr\);/s,
    )
  })

  it('fits the display editor into the remaining workspace height', () => {
    expect(stylesheet).toMatch(
      /\.workspace-scroll\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;/s,
    )
    expect(stylesheet).toMatch(
      /\.display-page\s*\{[^}]*display:\s*flex;[^}]*min-height:\s*0;[^}]*flex:\s*1\s+1\s+0;/s,
    )
    expect(stylesheet).toMatch(
      /\.display-workbench\s*\{[^}]*min-height:\s*0;[^}]*flex:\s*1\s+1\s+0;[^}]*overflow:\s*hidden;/s,
    )
  })

  it('contains scrolling inside both workbench columns', () => {
    expect(stylesheet).toMatch(
      /\.selected-list,\s*\n\.metric-picker\s*\{[^}]*overflow-y:\s*auto;[^}]*overscroll-behavior-y:\s*contain;[^}]*scrollbar-gutter:\s*stable;/s,
    )
    expect(stylesheet).toMatch(
      /\.list-heading,\s*\n\.metric-picker-heading\s*\{[^}]*position:\s*sticky;[^}]*top:\s*0;/s,
    )
  })

  it('drops a note separator wherever it would double up or float alone', () => {
    // 页首的说明上方没有内容，那条线会悬空
    expect(stylesheet).toMatch(
      /\.settings-page > \.section-note:first-child\s*\{[^}]*border-top:\s*0;/s,
    )
    // 紧跟已有分隔线的说明，再画一条就是平行双线
    expect(stylesheet).toMatch(
      /\.page-toolbar \+ \.section-note,\s*\n\.section-note \+ \.section-note\s*\{[^}]*border-top:\s*0;/s,
    )
    // 合计卡里的待加载文案是占位不是脚注，画线会和下面公式那条撞成双线
    expect(stylesheet).toMatch(
      /\.position-total > \.section-note\s*\{[^}]*border-top:\s*0;/s,
    )
  })

  it('falls back to natural page scrolling when the columns stack', () => {
    expect(stylesheet).toMatch(
      /@media \(max-width:\s*660px\)[\s\S]*?\.workspace-scroll\s*\{[^}]*display:\s*block;/,
    )
    expect(stylesheet).toMatch(
      /@media \(max-width:\s*660px\)[\s\S]*?\.selected-list,\s*\n\s*\.metric-picker\s*\{[^}]*overflow:\s*visible;/,
    )
  })
})
