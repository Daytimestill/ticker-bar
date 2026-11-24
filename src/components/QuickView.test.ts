// @vitest-environment jsdom

import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import QuickView from './QuickView.vue'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}))

const snapshot = {
  shortcut: 'Control+Command+T',
  connection: 'live',
  rows: [
    {
      symbol: '600756.SH',
      shortName: '浪潮软件',
      currency: 'CNY',
      lastPrice: '14.82',
      dailyChangePercent: '2.348',
      positionProfit: '1208',
      positionReturnPercent: '11.5',
      connection: 'live',
    },
    {
      symbol: '000977.SZ',
      shortName: '浪潮信息',
      currency: 'CNY',
      lastPrice: '52.16',
      dailyChangePercent: '-0.74',
      positionProfit: null,
      positionReturnPercent: null,
      connection: 'closed',
    },
  ],
  totals: [
    {
      currency: 'CNY',
      marketValue: '80680',
      costBasis: '78193.7',
      unrealizedProfit: '2486.3',
      returnPercent: '3.18',
    },
  ],
  missingQuotes: 0,
  updatedAt: '10:23:06',
  lastSuccessAt: '10:23',
  lastError: null,
}

describe('QuickView', () => {
  beforeEach(() => {
    invoke.mockReset()
    invoke.mockImplementation((command: string) => {
      if (command === 'get_quick_view_snapshot') {
        return Promise.resolve(snapshot)
      }
      if (command === 'hide_quick_view' || command === 'open_settings') {
        return Promise.resolve(undefined)
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders total profit and every configured stock from the cached snapshot', async () => {
    const wrapper = mount(QuickView)
    await flushPromises()

    expect(wrapper.get('[data-testid="quick-total-CNY"]').text()).toContain(
      '+2,486.30',
    )
    expect(wrapper.get('[data-testid="quick-total-CNY"]').text()).toContain(
      '↑3.18%',
    )
    expect(wrapper.get('[data-testid="quick-row-600756.SH"]').text()).toContain(
      '浪潮软件',
    )
    expect(wrapper.get('[data-testid="quick-row-600756.SH"]').text()).toContain(
      '+1,208.00',
    )
    expect(wrapper.get('[data-testid="quick-row-000977.SZ"]').text()).toContain(
      '—',
    )
    expect(wrapper.text()).toContain('10:23:06')
  })

  it('closes with Escape and opens the full settings window from the menu button', async () => {
    const wrapper = mount(QuickView)
    await flushPromises()

    const menuButton = wrapper.get('[data-testid="quick-open-settings"]')
    expect(menuButton.find('svg').exists()).toBe(true)
    expect(menuButton.text()).toBe('')

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await menuButton.trigger('click')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('hide_quick_view')
    expect(invoke).toHaveBeenCalledWith('open_settings')
  })

  it('shows a quiet recoverable error instead of an empty panel', async () => {
    invoke.mockRejectedValueOnce(new Error('行情状态已损坏'))

    const wrapper = mount(QuickView)
    await flushPromises()

    expect(wrapper.get('[data-testid="quick-error"]').text()).toContain(
      '暂时无法读取行情',
    )
    expect(wrapper.find('[data-testid="quick-retry"]').exists()).toBe(true)
  })
})
