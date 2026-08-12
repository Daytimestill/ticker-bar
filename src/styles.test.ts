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
      /\.settings-layout\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*190px\s+minmax\(0,\s*1fr\);/s,
    )
    expect(stylesheet).toMatch(
      /\.display-workbench\s*\{[^}]*display:\s*grid;[^}]*grid-template-columns:\s*minmax\(0,\s*1\.6fr\)\s+minmax\(260px,\s*1fr\);/s,
    )
  })

  it('uses one readable type and control-size system', () => {
    expect(stylesheet).toMatch(/--font-body:\s*13px;/)
    expect(stylesheet).toMatch(/--control-height:\s*40px;/)
    expect(stylesheet).toMatch(/--control-height-compact:\s*34px;/)
    expect(stylesheet).toMatch(
      /select\s*\{[^}]*appearance:\s*none;[^}]*background-image:\s*url\(/s,
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
