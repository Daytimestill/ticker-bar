// @vitest-environment jsdom

import { flushPromises, mount } from '@vue/test-utils'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App.vue'
import { createDefaultConfig } from './settings'

const invoke = vi.fn()

function setCardRect(element: Element, top: number) {
  vi.spyOn(element, 'getBoundingClientRect').mockReturnValue({
    x: 0,
    y: top,
    top,
    right: 600,
    bottom: top + 64,
    left: 0,
    width: 600,
    height: 64,
    toJSON: () => ({}),
  })
}

function dispatchPointer(
  element: Element,
  type: 'pointerdown' | 'pointermove' | 'pointerup' | 'pointercancel',
  clientX: number,
  clientY: number,
) {
  const event = new MouseEvent(type, {
    bubbles: true,
    cancelable: true,
    button: 0,
    clientX,
    clientY,
  })
  Object.defineProperty(event, 'pointerId', { value: 1 })
  element.dispatchEvent(event)
}

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}))

// 后端事件的假信道：测试直接调 emitAlert 就等于后端广播了一次
const alertHandlers: ((event: { payload: unknown }) => void)[] = []
const unlistenAlert = vi.fn()

vi.mock('@tauri-apps/api/event', () => ({
  listen: (event: string, handler: (event: { payload: unknown }) => void) => {
    if (event === 'alert-triggered') alertHandlers.push(handler)
    return Promise.resolve(unlistenAlert)
  },
}))

function emitAlert(title: string, body: string) {
  for (const handler of alertHandlers) handler({ payload: { title, body } })
}

describe('TickerBar settings window', () => {
  beforeEach(() => {
    // 记住的分区存在 localStorage 里，用例之间必须隔离
    localStorage.clear()
    alertHandlers.length = 0
    unlistenAlert.mockClear()
    invoke.mockReset()
    invoke.mockImplementation((command: string) => {
      if (command === 'get_config') {
        return Promise.resolve(createDefaultConfig())
      }
      if (command === 'preview_title') {
        return Promise.resolve('42.85 ↑1.54%')
      }
      if (command === 'preview_portfolio') {
        return Promise.resolve({ rows: [], totals: [], missingQuotes: 0 })
      }
      if (command === 'save_user_config') {
        return Promise.resolve(createDefaultConfig())
      }
      if (command === 'get_refresh_status') {
        return Promise.resolve({ lastSuccessAt: '10:23', lastError: null })
      }
      if (command === 'get_first_run') {
        return Promise.resolve(false)
      }
      if (command === 'get_app_info') {
        return Promise.resolve({ version: '0.1.0', builtAt: '08-10 22:41' })
      }
      if (command === 'send_test_alert') {
        return Promise.resolve(undefined)
      }
      if (command === 'dismiss_first_run') {
        return Promise.resolve(undefined)
      }
      if (command === 'search_stocks') {
        return Promise.resolve([
          {
            symbol: '600756.SH',
            name: '浪潮软件',
            market: '沪市',
            currency: 'CNY',
          },
        ])
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
  })

  it('loads the saved configuration and shows a live preview', async () => {
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.get('h1').text()).toBe('TickerBar')
    expect(wrapper.get('[data-testid="workspace-title"]').text()).toBe(
      '股票与行情',
    )
    expect(wrapper.get('[data-testid="tray-preview"]').text()).toBe(
      '42.85 ↑1.54%',
    )
    expect(
      (wrapper.get('[data-testid="metric-lastPrice"]').element as HTMLInputElement)
        .checked,
    ).toBe(true)
  })

  it('guides a first-time user and lands on the stock section afterwards', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'get_config') return Promise.resolve(createDefaultConfig())
      if (command === 'preview_title') return Promise.resolve('42.85')
      if (command === 'get_first_run') return Promise.resolve(true)
      if (command === 'dismiss_first_run') return Promise.resolve(undefined)
      if (command === 'get_refresh_status') {
        return Promise.resolve({ lastSuccessAt: null, lastError: null })
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
    const wrapper = mount(App)
    await flushPromises()

    const onboarding = wrapper.get('[data-testid="onboarding"]')
    expect(onboarding.text()).toContain('欢迎使用 TickerBar')
    expect(onboarding.text()).toContain('腾讯免费接口')
    expect(onboarding.text()).toContain('不读取任何券商账户')

    // 真模态：对话框声明 aria-modal，背景表单 inert 阻断 Tab 穿透
    expect(
      onboarding.get('[role="dialog"]').attributes('aria-modal'),
    ).toBe('true')
    expect(wrapper.get('form').attributes('inert')).toBeDefined()

    await wrapper.get('[data-testid="onboarding-start"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="onboarding"]').exists()).toBe(false)
    expect(wrapper.get('form').attributes('inert')).toBeUndefined()
    expect(wrapper.get('[data-testid="nav-stock"]').classes()).toContain(
      'is-active',
    )
    expect(invoke).toHaveBeenCalledWith('dismiss_first_run')
  })

  it('does not show the onboarding overlay on a normal launch', async () => {
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.find('[data-testid="onboarding"]').exists()).toBe(false)
  })

  it('shows the last successful refresh time and surfaces the last error', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(App)
      await flushPromises()

      expect(wrapper.get('[data-testid="refresh-status"]').text()).toContain(
        '最后更新 10:23',
      )
      expect(
        wrapper.find('[data-testid="refresh-status-error"]').exists(),
      ).toBe(false)

      invoke.mockImplementation((command: string) => {
        if (command === 'get_refresh_status') {
          return Promise.resolve({
            lastSuccessAt: '10:23',
            lastError: '10:25 quote request failed: timeout',
          })
        }
        return Promise.reject(new Error(`unexpected command: ${command}`))
      })
      // 轮询间隔 5 秒，推进时间让状态错误浮出
      await vi.advanceTimersByTimeAsync(5_500)
      await flushPromises()
      expect(
        wrapper.get('[data-testid="refresh-status-error"]').text(),
      ).toContain('10:25 quote request failed: timeout')
    } finally {
      vi.useRealTimers()
    }
  })

  it('首次打开落在股票页，并从侧边栏切换分区', async () => {
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.get('[data-testid="workspace-title"]').text()).toBe(
      '股票与行情',
    )
    expect(wrapper.get('[data-testid="nav-stock"]').classes()).toContain(
      'is-active',
    )
    expect(wrapper.get('[data-testid="page-stock"]').text()).toContain(
      '腾讯行情 · 无需密钥',
    )
    expect(wrapper.find('[data-testid="stock-search-input"]').exists()).toBe(true)
    expect(
      wrapper.get('[data-testid="page-stock"]').attributes('style'),
    ).not.toBe('display: none;')
    expect(
      wrapper.get('[data-testid="page-display"]').attributes('style'),
    ).toBe('display: none;')

    await wrapper.get('[data-testid="nav-display"]').trigger('click')

    expect(wrapper.get('[data-testid="workspace-title"]').text()).toBe(
      '菜单栏展示',
    )
    expect(wrapper.get('[data-testid="nav-display"]').classes()).toContain(
      'is-active',
    )
    expect(
      wrapper.get('[data-testid="page-display"]').attributes('style'),
    ).not.toBe('display: none;')
    expect(wrapper.get('[data-testid="page-stock"]').attributes('style')).toBe(
      'display: none;',
    )
  })

  it('重新打开设置时停在上次待过的分区', async () => {
    const first = mount(App)
    await flushPromises()
    await first.get('[data-testid="nav-alerts"]').trigger('click')
    expect(first.get('[data-testid="workspace-title"]').text()).toBe('提醒')
    first.unmount()

    // 关掉设置窗再打开，应当回到「提醒」而不是每次都固定跳同一页
    const reopened = mount(App)
    await flushPromises()
    expect(reopened.get('[data-testid="workspace-title"]').text()).toBe('提醒')
    expect(reopened.get('[data-testid="nav-alerts"]').classes()).toContain(
      'is-active',
    )
  })

  it('exposes both display columns as keyboard-accessible scroll regions', async () => {
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.find('[data-testid="display-workbench"]').exists()).toBe(true)
    const order = wrapper.get('[data-testid="display-order-scroll"]')
    const picker = wrapper.get('[data-testid="available-data-scroll"]')

    expect(order.attributes()).toMatchObject({
      role: 'region',
      'aria-label': '菜单栏显示顺序',
      tabindex: '0',
    })
    expect(picker.attributes()).toMatchObject({
      role: 'region',
      'aria-label': '可添加的数据',
      tabindex: '0',
    })
  })

  it('searches for a stock, selects it, and saves the canonical symbol', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')
    vi.useFakeTimers()

    try {
      await wrapper.get('[data-testid="stock-search-input"]').setValue('浪潮软件')
      await vi.advanceTimersByTimeAsync(300)
      await flushPromises()

      expect(invoke).toHaveBeenCalledWith('search_stocks', {
        query: '浪潮软件',
      })
      const option = wrapper.get(
        '[data-testid="stock-search-option-600756.SH"]',
      )
      expect(option.text()).toContain('浪潮软件')
      expect(option.text()).toContain('600756.SH')
      expect(option.text()).toContain('沪市')

      await option.trigger('click')
      // 选中即加入股票列表并置顶
      const row = wrapper.get('[data-testid="stock-row-600756.SH"]')
      expect(row.text()).toContain('浪潮软件')
      expect(row.find('[data-testid="active-stock-badge"]').exists()).toBe(true)
      expect(
        (wrapper.get('[data-testid="stock-short-name"]').element as HTMLInputElement)
          .value,
      ).toBe('浪潮软件')
      expect(
        (wrapper.get('[data-testid="stock-currency"]').element as HTMLInputElement)
          .value,
      ).toBe('CNY')

      await wrapper.get('form').trigger('submit')
      await flushPromises()
      expect(invoke).toHaveBeenCalledWith(
        'save_user_config',
        expect.objectContaining({
          config: expect.objectContaining({
            activeStock: 1,
            stocks: [
              expect.objectContaining({ symbol: '01810.HK' }),
              expect.objectContaining({
                symbol: '600756.SH',
                shortName: '浪潮软件',
                currency: 'CNY',
              }),
            ],
          }),
        }),
      )
    } finally {
      vi.useRealTimers()
    }
  })

  it('switches the pinned stock and removes an inactive stock from the list', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')
    vi.useFakeTimers()

    try {
      await wrapper.get('[data-testid="stock-search-input"]').setValue('浪潮软件')
      await vi.advanceTimersByTimeAsync(300)
      await flushPromises()
      await wrapper
        .get('[data-testid="stock-search-option-600756.SH"]')
        .trigger('click')

      // 新增股票自动置顶，点击旧股票行可切回
      await wrapper.get('[data-testid="stock-row-01810.HK"]').trigger('click')
      expect(
        wrapper
          .get('[data-testid="stock-row-01810.HK"]')
          .find('[data-testid="active-stock-badge"]')
          .exists(),
      ).toBe(true)

      // 移除非置顶股票
      await wrapper.get('[data-testid="stock-remove-600756.SH"]').trigger('click')
      expect(wrapper.find('[data-testid="stock-row-600756.SH"]').exists()).toBe(
        false,
      )
      expect(wrapper.find('[data-testid="stock-row-01810.HK"]').exists()).toBe(
        true,
      )
    } finally {
      vi.useRealTimers()
    }
  })

  it('置顶标记跟着股票名走，操作列宽度不随行变化', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')

    const row = wrapper.get('[data-testid="stock-row-01810.HK"]')
    const badge = row.get('[data-testid="active-stock-badge"]')
    // 标记在股票名那一格里，不在末尾的操作列——否则两种行末列宽度不一会顶歪整行
    expect(badge.element.closest('.stock-row-title')).not.toBeNull()
    expect(badge.element.closest('.stock-row-actions')).toBeNull()
    expect(row.find('.stock-row-actions').exists()).toBe(true)
    // 行内说明只留中文币种名，代码已在旁边单独一列
    expect(row.text()).toContain('港币行情')
  })

  it('展示版本与构建时间，便于确认覆盖安装是否生效', async () => {
    const wrapper = mount(App)
    await flushPromises()

    const build = wrapper.get('[data-testid="app-build"]')
    expect(build.text()).toContain('v0.1.0')
    expect(build.text()).toContain('构建于 08-10 22:41')
  })

  it('半填的行就地轻提示，不在页面底部堆红色横幅', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(App)
      await flushPromises()
      await wrapper.get('[data-testid="nav-position"]').trigger('click')

      await wrapper
        .get('[data-testid="position-quantity-01810.HK"]')
        .setValue('300')
      await vi.advanceTimersByTimeAsync(300)

      // 出错的那一行被标记，缺的那一格被描出来
      expect(
        wrapper.get('[data-testid="position-row-01810.HK"]').classes(),
      ).toContain('has-error')
      expect(
        wrapper
          .get('[data-testid="position-average-cost-01810.HK"]')
          .classes(),
      ).toContain('is-invalid')
      expect(
        wrapper.get('[data-testid="position-quantity-01810.HK"]').classes(),
      ).not.toContain('is-invalid')

      // 边填边报不该往顶栏塞红字，顶栏只在真正点保存时才报错
      expect(wrapper.find('.workspace-actions .error').exists()).toBe(false)

      await wrapper
        .get('[data-testid="position-average-cost-01810.HK"]')
        .setValue('39.46')
      await vi.advanceTimersByTimeAsync(300)
      expect(
        wrapper.get('[data-testid="position-row-01810.HK"]').classes(),
      ).not.toContain('has-error')
    } finally {
      vi.useRealTimers()
    }
  })

  it('可以调整股票顺序，且置顶不会串到别的股票', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')
    vi.useFakeTimers()

    try {
      await wrapper.get('[data-testid="stock-search-input"]').setValue('浪潮软件')
      await vi.advanceTimersByTimeAsync(300)
      await flushPromises()
      await wrapper
        .get('[data-testid="stock-search-option-600756.SH"]')
        .trigger('click')

      // 新加的浪潮软件自动置顶，此刻顺序是 [小米, 浪潮软件]
      const order = () =>
        wrapper
          .findAll('[data-testid^="stock-row-"]')
          .map((row) => row.attributes('data-testid'))
      expect(order()).toEqual(['stock-row-01810.HK', 'stock-row-600756.SH'])

      // 把置顶的浪潮软件上移一位
      await wrapper
        .get('[data-testid="stock-move-up-600756.SH"]')
        .trigger('click')
      expect(order()).toEqual(['stock-row-600756.SH', 'stock-row-01810.HK'])

      // 置顶仍是浪潮软件，没有因为下标位移换成小米
      expect(
        wrapper
          .get('[data-testid="stock-row-600756.SH"]')
          .find('[data-testid="active-stock-badge"]')
          .exists(),
      ).toBe(true)

      await wrapper.get('form').trigger('submit')
      await flushPromises()
      expect(invoke).toHaveBeenCalledWith(
        'save_user_config',
        expect.objectContaining({
          config: expect.objectContaining({
            activeStock: 0,
            stocks: [
              expect.objectContaining({ symbol: '600756.SH' }),
              expect.objectContaining({ symbol: '01810.HK' }),
            ],
          }),
        }),
      )
    } finally {
      vi.useRealTimers()
    }
  })

  it('refreshes the preview when the selected stock label or currency changes', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(App)
      await flushPromises()
      await wrapper.get('[data-testid="nav-stock"]').trigger('click')

      await wrapper.get('[data-testid="stock-short-name"]').setValue('浪潮')
      await wrapper.get('[data-testid="stock-currency"]').setValue('CNY')
      // 文本输入的预览请求做了防抖，推进时间才会真正发出。
      await vi.advanceTimersByTimeAsync(300)

      const previewCalls = invoke.mock.calls.filter(
        ([command]) => command === 'preview_title',
      )
      expect(previewCalls.at(-1)?.[1]).toMatchObject({
        config: {
          stocks: [
            {
              shortName: '浪潮',
              currency: 'CNY',
            },
          ],
        },
      })
    } finally {
      vi.useRealTimers()
    }
  })

  it('debounces preview requests while typing instead of firing per keystroke', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(App)
      await flushPromises()
      await wrapper.get('[data-testid="nav-stock"]').trigger('click')
      invoke.mockClear()

      const input = wrapper.get('[data-testid="stock-short-name"]')
      await input.setValue('浪')
      await input.setValue('浪潮')
      await input.setValue('浪潮软')
      await vi.advanceTimersByTimeAsync(300)

      const previewCalls = invoke.mock.calls.filter(
        ([command]) => command === 'preview_title',
      )
      expect(previewCalls).toHaveLength(1)
    } finally {
      vi.useRealTimers()
    }
  })

  it('can add a position metric from the available field list', async () => {
    const wrapper = mount(App)
    await flushPromises()

    await wrapper.get('[data-testid="metric-positionProfit"]').setValue(true)

    expect(wrapper.text()).toContain('持仓收益')
    expect(invoke).toHaveBeenCalledWith(
      'preview_title',
      expect.objectContaining({
        config: expect.objectContaining({
          display: expect.objectContaining({
            items: expect.arrayContaining([
              expect.objectContaining({ metric: 'positionProfit' }),
            ]),
          }),
        }),
      }),
    )
  })

  it('organizes available metrics into clear groups with visible selection state', async () => {
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.findAll('[data-testid^="metric-group-"]')).toHaveLength(3)
    expect(wrapper.get('[data-testid="metric-group-quote"]').text()).toContain(
      '行情',
    )
    expect(wrapper.get('[data-testid="metric-group-position"]').text()).toContain(
      '持仓',
    )
    expect(wrapper.get('[data-testid="metric-group-status"]').text()).toContain(
      '状态',
    )
    expect(wrapper.get('[data-testid="metric-selection-count"]').text()).toBe(
      '已选 2 项',
    )
    expect(wrapper.get('[data-testid="metric-card-lastPrice"]').classes()).toContain(
      'is-selected',
    )
    expect(
      wrapper.get('[data-testid="metric-card-positionProfit"]').classes(),
    ).not.toContain('is-selected')

    await wrapper.get('[data-testid="metric-positionProfit"]').setValue(true)

    expect(wrapper.get('[data-testid="metric-selection-count"]').text()).toBe(
      '已选 3 项',
    )
    expect(
      wrapper.get('[data-testid="metric-card-positionProfit"]').classes(),
    ).toContain('is-selected')
  })

  it('applies the position preset and persists the configuration', async () => {
    const wrapper = mount(App)
    await flushPromises()

    await wrapper.get('[data-testid="preset-position"]').trigger('click')
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith(
      'save_user_config',
      expect.objectContaining({
        config: expect.objectContaining({
          display: expect.objectContaining({
            items: [
              expect.objectContaining({ metric: 'positionProfit' }),
              expect.objectContaining({ metric: 'positionReturnPercent' }),
            ],
          }),
        }),
      }),
    )
    expect(wrapper.text()).toContain('设置已保存')
  })

  it('changes field order with buttons and pointer dragging', async () => {
    const wrapper = mount(App)
    await flushPromises()

    await wrapper
      .get('[data-testid="selected-lastPrice"] [aria-label="下移"]')
      .trigger('click')
    let previewCalls = invoke.mock.calls.filter(
      ([command]) => command === 'preview_title',
    )
    expect(previewCalls.at(-1)?.[1]).toMatchObject({
      config: {
        display: {
          items: [
            expect.objectContaining({ metric: 'dailyChangePercent' }),
            expect.objectContaining({ metric: 'lastPrice' }),
          ],
        },
      },
    })

    const lastPriceCard = wrapper.get('[data-testid="selected-lastPrice"]')
    const dailyChangeCard = wrapper.get(
      '[data-testid="selected-dailyChangePercent"]',
    )
    const handle = wrapper.get('[data-testid="drag-handle-lastPrice"]')
    setCardRect(dailyChangeCard.element, 0)
    setCardRect(lastPriceCard.element, 64)

    dispatchPointer(handle.element, 'pointerdown', 20, 96)
    dispatchPointer(handle.element, 'pointermove', 20, 32)
    await wrapper.vm.$nextTick()

    expect(document.querySelector('.drag-ghost')?.textContent).toContain('当前价格')
    expect(dailyChangeCard.classes()).toContain('is-drop-target')

    dispatchPointer(handle.element, 'pointerup', 20, 32)
    await flushPromises()

    previewCalls = invoke.mock.calls.filter(
      ([command]) => command === 'preview_title',
    )
    expect(previewCalls.at(-1)?.[1]).toMatchObject({
      config: {
        display: {
          items: [
            expect.objectContaining({ metric: 'lastPrice' }),
            expect.objectContaining({ metric: 'dailyChangePercent' }),
          ],
        },
      },
    })
  })

  it('does not start pointer dragging before the movement threshold', async () => {
    const wrapper = mount(App)
    await flushPromises()
    const card = wrapper.get('[data-testid="selected-lastPrice"]')
    const handle = wrapper.get('[data-testid="drag-handle-lastPrice"]')
    setCardRect(card.element, 0)

    expect(card.attributes('draggable')).toBeUndefined()
    expect(handle.attributes('draggable')).toBeUndefined()

    dispatchPointer(handle.element, 'pointerdown', 20, 20)
    dispatchPointer(handle.element, 'pointermove', 22, 22)
    dispatchPointer(handle.element, 'pointerup', 22, 22)
    await wrapper.vm.$nextTick()

    expect(document.querySelector('.drag-ghost')).toBeNull()
    expect(wrapper.find('.is-dragging').exists()).toBe(false)
    expect(wrapper.find('.is-drop-target').exists()).toBe(false)
  })

  it('cleans up an active pointer drag when the system cancels it', async () => {
    const wrapper = mount(App)
    await flushPromises()
    const card = wrapper.get('[data-testid="selected-lastPrice"]')
    const handle = wrapper.get('[data-testid="drag-handle-lastPrice"]')
    const releasePointerCapture = vi.fn()
    setCardRect(card.element, 0)
    Object.defineProperties(handle.element, {
      setPointerCapture: { value: vi.fn(), configurable: true },
      hasPointerCapture: {
        value: vi.fn().mockReturnValue(true),
        configurable: true,
      },
      releasePointerCapture: {
        value: releasePointerCapture,
        configurable: true,
      },
    })

    dispatchPointer(handle.element, 'pointerdown', 20, 20)
    dispatchPointer(handle.element, 'pointermove', 20, 48)
    await wrapper.vm.$nextTick()
    expect(document.querySelector('.drag-ghost')).not.toBeNull()
    expect(card.classes()).toContain('is-dragging')

    dispatchPointer(handle.element, 'pointercancel', 20, 48)
    await wrapper.vm.$nextTick()

    expect(releasePointerCapture).toHaveBeenCalledWith(1)
    expect(document.querySelector('.drag-ghost')).toBeNull()
    expect(wrapper.find('.is-dragging').exists()).toBe(false)
  })

  it('updates precision, format, compact style, and a short label', async () => {
    const wrapper = mount(App)
    await flushPromises()

    // 持仓收益是少数三项调整全都适用的指标（可正可负、可达万级）
    await wrapper.get('[data-testid="metric-positionProfit"]').setValue(true)
    await wrapper
      .get('[data-testid="item-toggle-positionProfit"]')
      .trigger('click')
    await wrapper
      .get('[data-testid="precision-positionProfit-1"]')
      .trigger('click')
    await wrapper
      .get('[data-testid="format-positionProfit-sign"]')
      .trigger('click')
    await wrapper
      .get('[data-testid="compact-positionProfit-western"]')
      .trigger('click')
    await wrapper.get('[data-testid="label-positionProfit"]').setValue('盈')
    await flushPromises()

    const previewCalls = invoke.mock.calls.filter(
      ([command]) => command === 'preview_title',
    )
    expect(previewCalls.at(-1)?.[1]).toMatchObject({
      config: {
        display: {
          items: expect.arrayContaining([
            expect.objectContaining({
              metric: 'positionProfit',
              precision: 1,
              showSign: true,
              directionArrow: false,
              compactStyle: 'western',
              label: '盈',
            }),
          ]),
        },
      },
    })
  })

  it('hides advanced metric formatting until the user asks to adjust it', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="metric-positionProfit"]').setValue(true)

    expect(
      wrapper
        .get('[data-testid="item-options-positionProfit"]')
        .attributes('style'),
    ).toBe('display: none;')

    await wrapper
      .get('[data-testid="item-toggle-positionProfit"]')
      .trigger('click')

    expect(
      wrapper
        .get('[data-testid="item-options-positionProfit"]')
        .attributes('style'),
    ).not.toBe('display: none;')
    expect(
      wrapper.findAll('[data-testid^="precision-positionProfit-"]'),
    ).toHaveLength(4)
    expect(
      wrapper.findAll('[data-testid^="format-positionProfit-"]'),
    ).toHaveLength(3)
    expect(
      wrapper.findAll('[data-testid^="compact-positionProfit-"]'),
    ).toHaveLength(3)
  })

  it('只为真正适用的显示项提供数值调整', async () => {
    const wrapper = mount(App)
    await flushPromises()

    // 市场状态是纯文本：小数位/正负号/缩写一个都不该出现，只留短标签
    await wrapper.get('[data-testid="metric-marketStatus"]').setValue(true)
    await wrapper
      .get('[data-testid="item-toggle-marketStatus"]')
      .trigger('click')
    expect(
      wrapper.findAll('[data-testid^="precision-marketStatus-"]'),
    ).toHaveLength(0)
    expect(
      wrapper.findAll('[data-testid^="format-marketStatus-"]'),
    ).toHaveLength(0)
    expect(
      wrapper.findAll('[data-testid^="compact-marketStatus-"]'),
    ).toHaveLength(0)
    expect(wrapper.find('[data-testid="label-marketStatus"]').exists()).toBe(
      true,
    )
    expect(
      wrapper.get('[data-testid="item-text-only-marketStatus"]').text(),
    ).toContain('文本内容')

    // 价格恒为正：有小数位可调，但正负号/箭头没有意义
    await wrapper.get('[data-testid="item-toggle-lastPrice"]').trigger('click')
    expect(
      wrapper.findAll('[data-testid^="precision-lastPrice-"]'),
    ).toHaveLength(4)
    expect(wrapper.findAll('[data-testid^="format-lastPrice-"]')).toHaveLength(
      0,
    )
    expect(wrapper.findAll('[data-testid^="compact-lastPrice-"]')).toHaveLength(
      0,
    )
  })

  it('计算收益无需额外开关：填入数量与成本即生效，清空即停止计算', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(App)
      await flushPromises()
      await wrapper.get('[data-testid="nav-position"]').trigger('click')

      // 首启无持仓：输入框直接可用，但不展示编造的盈亏数据
      const quantity = wrapper.get(
        '[data-testid="position-quantity-01810.HK"]',
      )
      const averageCost = wrapper.get(
        '[data-testid="position-average-cost-01810.HK"]',
      )
      expect((quantity.element as HTMLInputElement).value).toBe('')
      expect(
        wrapper.get('[data-testid="position-result-01810.HK"]').text(),
      ).toContain('未计算')

      await quantity.setValue('300')
      await averageCost.setValue('40')
      await vi.advanceTimersByTimeAsync(300)

      const previewCalls = invoke.mock.calls.filter(
        ([command]) => command === 'preview_title',
      )
      expect(previewCalls.at(-1)?.[1]).toMatchObject({
        config: {
          stocks: [{ position: { quantity: '300', averageCost: '40' } }],
        },
      })

      // 两项都清空 = 不计算收益，配置回到 null
      await quantity.setValue('')
      await averageCost.setValue('')
      await vi.advanceTimersByTimeAsync(300)

      const latest = invoke.mock.calls
        .filter(([command]) => command === 'preview_title')
        .at(-1)?.[1] as { config: { stocks: { position: unknown }[] } }
      expect(latest.config.stocks[0].position).toBeNull()
    } finally {
      vi.useRealTimers()
    }
  })

  it('flags invalid position input and refuses to save it', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-position"]').trigger('click')

    await wrapper
      .get('[data-testid="position-quantity-01810.HK"]')
      .setValue('abc')
    await wrapper
      .get('[data-testid="position-average-cost-01810.HK"]')
      .setValue('40')
    expect(wrapper.get('[data-testid="position-error-01810.HK"]').text()).toContain(
      '持仓数量需要是非负数字',
    )

    invoke.mockClear()
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    const saveCalls = invoke.mock.calls.filter(
      ([command]) => command === 'save_user_config',
    )
    expect(saveCalls).toHaveLength(0)
    expect(wrapper.text()).toContain('持仓数量需要是非负数字')
  })

  it('半填持仓时不把空值发给后端，只给出中文提示', async () => {
    vi.useFakeTimers()
    try {
      const wrapper = mount(App)
      await flushPromises()
      await wrapper.get('[data-testid="nav-position"]').trigger('click')
      invoke.mockClear()

      // 只填成本不填数量：空字符串无法反序列化成 Decimal，绝不能发出去
      await wrapper
        .get('[data-testid="position-average-cost-01810.HK"]')
        .setValue('40')
      await vi.advanceTimersByTimeAsync(300)

      expect(
        invoke.mock.calls.filter(
          ([command]) =>
            command === 'preview_title' || command === 'preview_portfolio',
        ),
      ).toHaveLength(0)
      expect(wrapper.get('[data-testid="tray-preview"]').text()).toBe(
        '待补全设置',
      )
      expect(wrapper.text()).toContain('持仓数量与平均成本需要同时填写')
      expect(wrapper.text()).not.toContain('Decimal')

      // 补齐后恢复正常预览
      await wrapper
        .get('[data-testid="position-quantity-01810.HK"]')
        .setValue('100')
      await vi.advanceTimersByTimeAsync(300)
      expect(
        invoke.mock.calls.filter(([command]) => command === 'preview_title'),
      ).not.toHaveLength(0)
    } finally {
      vi.useRealTimers()
    }
  })

  it('可以一键放弃未保存的修改，回到上次保存的状态', async () => {
    const wrapper = mount(App)
    await flushPromises()

    const discard = wrapper.get('[data-testid="discard"]')
    expect(discard.attributes('disabled')).toBeDefined()
    expect(wrapper.find('[data-testid="dirty-hint"]').exists()).toBe(false)

    // 改一项显示设置：出现未保存提示，放弃按钮可用
    await wrapper.get('[data-testid="metric-positionProfit"]').setValue(true)
    await flushPromises()
    expect(wrapper.get('[data-testid="dirty-hint"]').text()).toContain(
      '有未保存的修改',
    )
    expect(
      wrapper.get('[data-testid="discard"]').attributes('disabled'),
    ).toBeUndefined()

    await wrapper.get('[data-testid="discard"]').trigger('click')
    await flushPromises()

    // 还原后回到初始的两项，且不再提示未保存
    expect(wrapper.get('[data-testid="metric-selection-count"]').text()).toBe(
      '已选 2 项',
    )
    expect(
      (
        wrapper.get('[data-testid="metric-positionProfit"]')
          .element as HTMLInputElement
      ).checked,
    ).toBe(false)
    expect(wrapper.find('[data-testid="dirty-hint"]').exists()).toBe(false)
    expect(wrapper.text()).toContain('已还原到上次保存的设置')
  })

  it('交易货币是下拉选择而不是自由文本，并说明用途', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')

    const currency = wrapper.get('[data-testid="stock-currency"]')
    expect(currency.element.tagName).toBe('SELECT')
    expect((currency.element as HTMLSelectElement).value).toBe('HKD')
    expect(currency.text()).toContain('人民币 CNY')
    expect(currency.text()).toContain('港币 HKD')
    expect(wrapper.get('[data-testid="page-stock"]').text()).toContain(
      '仅用于持仓收益的币种归类',
    )
  })

  it('只填数量不填成本时拦下保存，提示两项必须同时填写', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-position"]').trigger('click')

    await wrapper
      .get('[data-testid="position-quantity-01810.HK"]')
      .setValue('300')

    expect(wrapper.get('[data-testid="position-error-01810.HK"]').text()).toContain(
      '持仓数量与平均成本需要同时填写',
    )

    invoke.mockClear()
    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(
      invoke.mock.calls.filter(([command]) => command === 'save_user_config'),
    ).toHaveLength(0)
  })

  it('逐股显示收益并按币种分组展示合计', async () => {
    const configured = createDefaultConfig()
    configured.stocks[0].position = { quantity: '250', averageCost: '39.46' }
    invoke.mockImplementation((command: string) => {
      if (command === 'get_config') return Promise.resolve(configured)
      if (command === 'preview_title') return Promise.resolve('42.85')
      if (command === 'get_first_run') return Promise.resolve(false)
      if (command === 'get_refresh_status') {
        return Promise.resolve({ lastSuccessAt: '10:23', lastError: null })
      }
      if (command === 'preview_portfolio') {
        return Promise.resolve({
          rows: [
            {
              symbol: '01810.HK',
              shortName: '小米',
              currency: 'HKD',
              marketValue: '6845.00',
              costBasis: '9865.00',
              unrealizedProfit: '-3020.00',
              returnPercent: '-30.61',
            },
          ],
          totals: [
            {
              currency: 'HKD',
              marketValue: '6845.00',
              costBasis: '9865.00',
              unrealizedProfit: '-3020.00',
              returnPercent: '-30.61',
            },
            {
              currency: 'CNY',
              marketValue: '2200.00',
              costBasis: '2000.00',
              unrealizedProfit: '200.00',
              returnPercent: '10.00',
            },
          ],
          missingQuotes: 1,
        })
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-position"]').trigger('click')

    expect(
      wrapper.get('[data-testid="position-result-01810.HK"]').text(),
    ).toContain('-3020.00')

    const total = wrapper.get('[data-testid="position-total"]')
    // 两种币分开列，绝不相加
    expect(total.get('[data-testid="position-total-HKD"]').text()).toContain(
      '-3020.00',
    )
    expect(total.get('[data-testid="position-total-CNY"]').text()).toContain(
      '+200.00',
    )
    // 合计要讲清楚是怎么算出来的、算了几只、差了几只
    expect(total.text()).toContain('市值 6845.00 · 成本 9865.00')
    expect(total.text()).toContain('收益 = Σ（现价 − 成本）× 数量')
    expect(total.text()).toContain('合计收益 ÷ 合计成本')
    expect(total.text()).toContain('另有 1 只股票暂无行情，未计入')
  })

  it('keeps editing the same stock position after another stock is removed', async () => {
    // 老实现按下标记「正在编辑哪只」，删掉前面的股票会静默串到别的股票。
    invoke.mockImplementation((command: string, payload?: unknown) => {
      if (command === 'get_config') return Promise.resolve(createDefaultConfig())
      if (command === 'preview_title') return Promise.resolve('42.85')
      if (command === 'get_first_run') return Promise.resolve(false)
      if (command === 'get_refresh_status') {
        return Promise.resolve({ lastSuccessAt: null, lastError: null })
      }
      if (command === 'search_stocks') {
        const { query } = payload as { query: string }
        if (query.includes('浪潮')) {
          return Promise.resolve([
            { symbol: '600756.SH', name: '浪潮软件', market: '沪市', currency: 'CNY' },
          ])
        }
        return Promise.resolve([
          { symbol: '000001.SZ', name: '平安银行', market: '深市', currency: 'CNY' },
        ])
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')
    vi.useFakeTimers()

    try {
      const search = wrapper.get('[data-testid="stock-search-input"]')
      await search.setValue('浪潮软件')
      await vi.advanceTimersByTimeAsync(300)
      await flushPromises()
      await wrapper
        .get('[data-testid="stock-search-option-600756.SH"]')
        .trigger('click')
      await search.setValue('平安银行')
      await vi.advanceTimersByTimeAsync(300)
      await flushPromises()
      await wrapper
        .get('[data-testid="stock-search-option-000001.SZ"]')
        .trigger('click')

      // 持仓页给中间的浪潮软件配持仓
      await wrapper.get('[data-testid="nav-position"]').trigger('click')
      await wrapper
        .get('[data-testid="position-quantity-600756.SH"]')
        .setValue('500')
      await wrapper
        .get('[data-testid="position-average-cost-600756.SH"]')
        .setValue('20')

      // 回股票页删掉排在它前面的小米（非置顶，可删）
      await wrapper.get('[data-testid="nav-stock"]').trigger('click')
      await wrapper.get('[data-testid="stock-remove-01810.HK"]').trigger('click')

      // 持仓仍然挂在浪潮软件上，不会因下标前移而串到别的股票
      await wrapper.get('[data-testid="nav-position"]').trigger('click')
      expect(
        (
          wrapper.get('[data-testid="position-quantity-600756.SH"]')
            .element as HTMLInputElement
        ).value,
      ).toBe('500')
      await vi.advanceTimersByTimeAsync(300)

      const previewCalls = invoke.mock.calls.filter(
        ([command]) => command === 'preview_title',
      )
      const stocks = (
        previewCalls.at(-1)?.[1] as {
          config: { stocks: { symbol: string; position: unknown }[] }
        }
      ).config.stocks
      expect(
        stocks.find((stock) => stock.symbol === '600756.SH')?.position,
      ).toMatchObject({ quantity: '500', averageCost: '20' })
      expect(
        stocks.find((stock) => stock.symbol === '000001.SZ')?.position,
      ).toBeNull()
    } finally {
      vi.useRealTimers()
    }
  })

  it('removes alerts referencing a stock when that stock is removed', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-stock"]').trigger('click')
    vi.useFakeTimers()

    try {
      await wrapper.get('[data-testid="stock-search-input"]').setValue('浪潮软件')
      await vi.advanceTimersByTimeAsync(300)
      await flushPromises()
      await wrapper
        .get('[data-testid="stock-search-option-600756.SH"]')
        .trigger('click')

      // 给刚置顶的浪潮软件建一条提醒
      await wrapper.get('[data-testid="nav-alerts"]').trigger('click')
      await wrapper.get('[data-testid="alert-create"]').trigger('click')
      await wrapper.get('[data-testid="alert-threshold"]').setValue('30')
      await wrapper.get('[data-testid="alert-metric"]').setValue('price')
      await wrapper.get('[data-testid="alert-submit"]').trigger('click')
      expect(wrapper.get('[data-testid="alert-list"]').text()).toContain('浪潮软件')

      // 换回小米置顶后删除浪潮软件：提醒规则应一并清理
      await wrapper.get('[data-testid="nav-stock"]').trigger('click')
      await wrapper.get('[data-testid="stock-row-01810.HK"]').trigger('click')
      await wrapper.get('[data-testid="stock-remove-600756.SH"]').trigger('click')

      await wrapper.get('[data-testid="nav-alerts"]').trigger('click')
      expect(wrapper.find('[data-testid="alert-list"]').exists()).toBe(false)
    } finally {
      vi.useRealTimers()
    }
  })

  it('blocks saving when a saved alert depends on a disabled position', async () => {
    const wrapper = mount(App)
    await flushPromises()

    // 先配好持仓并建一条持仓收益提醒
    await wrapper.get('[data-testid="nav-position"]').trigger('click')
    await wrapper
      .get('[data-testid="position-quantity-01810.HK"]')
      .setValue('100')
    await wrapper
      .get('[data-testid="position-average-cost-01810.HK"]')
      .setValue('10')
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')
    await wrapper.get('[data-testid="alert-create"]').trigger('click')
    await wrapper.get('[data-testid="alert-metric"]').setValue('positionProfit')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('-2000')
    await wrapper.get('[data-testid="alert-comparator-below"]').trigger('click')
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')
    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain('持仓收益')

    // 再清空持仓：规则变成永不触发的死规则，保存必须被拦下
    await wrapper.get('[data-testid="nav-position"]').trigger('click')
    await wrapper.get('[data-testid="position-quantity-01810.HK"]').setValue('')
    await wrapper
      .get('[data-testid="position-average-cost-01810.HK"]')
      .setValue('')
    invoke.mockClear()
    await wrapper.get('form').trigger('submit')
    await flushPromises()

    const saveCalls = invoke.mock.calls.filter(
      ([command]) => command === 'save_user_config',
    )
    expect(saveCalls).toHaveLength(0)
    expect(wrapper.text()).toContain('未启用持仓计算')
  })

  it('creates a disguised alert rule and includes it in the saved config', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')

    await wrapper.get('[data-testid="alert-create"]').trigger('click')
    await wrapper.get('[data-testid="alert-metric"]').setValue('changePercent')
    await wrapper.get('[data-testid="alert-comparator-above"]').trigger('click')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('3')
    await wrapper.get('[data-testid="alert-custom-title"]').setValue('今天吃了三斤肉')
    await wrapper.get('[data-testid="alert-silent"]').setValue(true)
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')

    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain(
      '小米 今日涨跌幅 ≥ 3%',
    )
    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain('伪装文案')
    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain('静默')

    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(invoke).toHaveBeenCalledWith(
      'save_user_config',
      expect.objectContaining({
        config: expect.objectContaining({
          alerts: [
            expect.objectContaining({
              symbol: '01810.HK',
              metric: 'changePercent',
              comparator: 'above',
              threshold: '3',
              repeat: 'dailyOnce',
              enabled: true,
              silent: true,
              customTitle: '今天吃了三斤肉',
              customBody: null,
            }),
          ],
        }),
      }),
    )
  })

  it('edits, toggles, and removes an existing alert rule', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')

    await wrapper.get('[data-testid="alert-create"]').trigger('click')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('30')
    await wrapper.get('[data-testid="alert-metric"]').setValue('price')
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')
    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain(
      '小米 股价 ≥ 30',
    )

    // 编辑：改阈值并保存
    await wrapper.get('[data-testid^="alert-edit-"]').trigger('click')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('35')
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')
    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain(
      '小米 股价 ≥ 35',
    )

    // 开关
    const toggle = wrapper.get('[data-testid^="alert-enabled-"]')
    await toggle.setValue(false)
    expect(wrapper.get('.alert-row').classes()).toContain('is-disabled')

    // 删除
    await wrapper.get('[data-testid^="alert-remove-"]').trigger('click')
    expect(wrapper.find('[data-testid="alert-list"]').exists()).toBe(false)
  })

  it('表单实时预览拼出来的规则，与列表里的描述同源', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')
    await wrapper.get('[data-testid="alert-create"]').trigger('click')

    // 阈值没填时无从预览，给占位提示而不是拼出半句话
    expect(wrapper.find('[data-testid="alert-draft-summary"]').exists()).toBe(false)

    await wrapper.get('[data-testid="alert-threshold"]').setValue('3')
    await wrapper.get('[data-testid="alert-comparator-below"]').trigger('click')

    const summary = wrapper.get('[data-testid="alert-draft-summary"]').text()
    expect(summary).toContain('≤')
    expect(summary).toContain('3%')

    // 提交后列表里那条描述必须和预览一字不差，否则预览就是在骗人
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')
    expect(wrapper.get('[data-testid="alert-list"]').text()).toContain(summary)
  })

  it('提醒页常驻说明通知的两种去向，没有规则时也在', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')

    // 空列表状态最容易漏——用户还没建规则时就该知道横幅会去哪
    expect(wrapper.find('[data-testid="alert-list"]').exists()).toBe(false)
    const note = wrapper.get('[data-testid="alert-delivery-note"]').text()
    expect(note).toContain('桌面横幅')
    expect(note).toContain('通知中心')
  })

  it('提醒触发时在窗口内弹 Toast，点一下关掉', async () => {
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.find('[data-testid="alert-toasts"]').exists()).toBe(false)

    emitAlert('小米集团-W 涨超 3%', '现价 42.85，今日 +3.21%')
    await flushPromises()

    const toasts = wrapper.get('[data-testid="alert-toasts"]')
    expect(toasts.text()).toContain('小米集团-W 涨超 3%')
    expect(toasts.text()).toContain('42.85')

    await wrapper.get('[data-testid^="alert-toast-"]').trigger('click')
    expect(wrapper.find('[data-testid="alert-toasts"]').exists()).toBe(false)
  })

  it('Toast 不在提醒页也会弹，因为触发时用户可能停在任何一页', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-display"]').trigger('click')

    emitAlert('腾讯控股 跌破 400', '现价 398.20')
    await flushPromises()

    expect(wrapper.get('[data-testid="alert-toasts"]').text()).toContain(
      '腾讯控股 跌破 400',
    )
  })

  it('连续触发只保留最近 4 条，不把窗口糊满', async () => {
    const wrapper = mount(App)
    await flushPromises()

    for (let index = 1; index <= 6; index += 1) {
      emitAlert(`提醒 ${index}`, `正文 ${index}`)
    }
    await flushPromises()

    const toasts = wrapper.findAll('[data-testid^="alert-toast-"]')
    expect(toasts).toHaveLength(4)
    // 挤掉的是最旧的两条，最新的必须还在
    expect(toasts[0].text()).toContain('提醒 3')
    expect(toasts[3].text()).toContain('提醒 6')
  })

  it('窗口销毁时注销事件监听，避免悬挂监听', async () => {
    const wrapper = mount(App)
    await flushPromises()

    wrapper.unmount()
    expect(unlistenAlert).toHaveBeenCalled()
  })

  it('可以试发提醒，休市时也能验证通知链路', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')

    await wrapper.get('[data-testid="alert-create"]').trigger('click')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('30')
    await wrapper.get('[data-testid="alert-metric"]').setValue('price')
    await wrapper.get('[data-testid="alert-custom-title"]').setValue('今天吃了三斤肉')
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')

    vi.useFakeTimers()
    await wrapper.get('[data-testid^="alert-test-"]').trigger('click')

    // 倒计时期间不发，先给用户留出切走窗口的时间
    expect(wrapper.get('[data-testid="alert-test-countdown"]').text()).toContain(
      '切到其他窗口',
    )
    expect(
      invoke.mock.calls.some(([command]) => command === 'send_test_alert'),
    ).toBe(false)

    await vi.advanceTimersByTimeAsync(3000)
    vi.useRealTimers()
    await flushPromises()

    // 试发把整条规则交给后端，由后端走与真实触发相同的文案组装与发送路径
    const testCall = invoke.mock.calls.find(
      ([command]) => command === 'send_test_alert',
    )
    expect(testCall?.[1]).toMatchObject({
      rule: expect.objectContaining({
        symbol: '01810.HK',
        metric: 'price',
        customTitle: '今天吃了三斤肉',
      }),
    })
    expect(wrapper.get('[data-testid="alert-test-result"]').text()).toContain(
      '已发送',
    )
  })

  it('试发失败时给出可操作的中文提示', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'get_config') return Promise.resolve(createDefaultConfig())
      if (command === 'preview_title') return Promise.resolve('42.85')
      if (command === 'preview_portfolio') {
        return Promise.resolve({ rows: [], totals: [], missingQuotes: 0 })
      }
      if (command === 'get_first_run') return Promise.resolve(false)
      if (command === 'get_app_info') {
        return Promise.resolve({ version: '0.1.0', builtAt: null })
      }
      if (command === 'get_refresh_status') {
        return Promise.resolve({ lastSuccessAt: null, lastError: null })
      }
      if (command === 'send_test_alert') {
        return Promise.reject(
          new Error('通知发送失败，请到 系统设置 → 通知 里允许 TickerBar 发送通知：denied'),
        )
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')

    await wrapper.get('[data-testid="alert-create"]').trigger('click')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('30')
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')
    vi.useFakeTimers()
    await wrapper.get('[data-testid^="alert-test-"]').trigger('click')
    await vi.advanceTimersByTimeAsync(3000)
    vi.useRealTimers()
    await flushPromises()

    expect(wrapper.get('[data-testid="alert-test-error"]').text()).toContain(
      '系统设置 → 通知',
    )
  })

  it('rejects an alert draft with a malformed threshold', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-alerts"]').trigger('click')

    await wrapper.get('[data-testid="alert-create"]').trigger('click')
    await wrapper.get('[data-testid="alert-threshold"]').setValue('abc')
    await wrapper.get('[data-testid="alert-submit"]').trigger('click')

    expect(wrapper.get('[data-testid="alert-form-error"]').text()).toBe(
      '阈值需要是数字（可为负数）',
    )
    expect(wrapper.find('[data-testid="alert-list"]').exists()).toBe(false)
  })

  it('系统设置只保留真正生效的开关，不留看似可用的摆设', async () => {
    const wrapper = mount(App)
    await flushPromises()
    await wrapper.get('[data-testid="nav-system"]').trigger('click')

    const runtimeSettings = wrapper.get('[data-testid="runtime-settings"]')
    const runtimeOptions = runtimeSettings.findAll('.runtime-option')

    expect(runtimeSettings.text()).toContain('运行选项')
    expect(runtimeOptions).toHaveLength(1)
    expect(runtimeOptions[0].find('input.toggle-input').exists()).toBe(true)
    expect(runtimeOptions[0].find('.runtime-copy').exists()).toBe(true)

    // 「扩展时段」始终无法生效，已整体移除而不是继续禁用着占位
    expect(
      wrapper.find('[data-testid="extended-hours-toggle"]').exists(),
    ).toBe(false)
    expect(wrapper.text()).not.toContain('扩展时段')
    expect(wrapper.text()).not.toContain('待支持')
  })

  it('shows validation feedback when removing the final metric', async () => {
    const onlyPrice = createDefaultConfig()
    onlyPrice.display.items = [onlyPrice.display.items[0]]
    invoke.mockImplementation((command: string) => {
      if (command === 'get_config') return Promise.resolve(onlyPrice)
      if (command === 'preview_title') return Promise.resolve('42.85')
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
    const wrapper = mount(App)
    await flushPromises()

    await wrapper.get('[data-testid="metric-lastPrice"]').setValue(false)

    expect(wrapper.text()).toContain('至少保留一个菜单栏数据项')
  })

  it('reports loading, preview, and save errors without losing the form', async () => {
    invoke.mockImplementation((command: string) => {
      if (command === 'get_config') return Promise.resolve(createDefaultConfig())
      if (command === 'preview_title') {
        return Promise.reject(new Error('行情暂不可用'))
      }
      if (command === 'save_user_config') {
        return Promise.reject(new Error('配置不可写'))
      }
      return Promise.reject(new Error(`unexpected command: ${command}`))
    })
    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.get('[data-testid="tray-preview"]').text()).toBe('预览不可用')
    expect(wrapper.text()).toContain('行情暂不可用')

    await wrapper.get('form').trigger('submit')
    await flushPromises()
    expect(wrapper.text()).toContain('配置不可写')
    expect(wrapper.get('[data-testid="save"]').attributes('disabled')).toBeUndefined()
  })

  it('reports a configuration loading error', async () => {
    invoke.mockRejectedValue(new Error('读取失败'))

    const wrapper = mount(App)
    await flushPromises()

    expect(wrapper.text()).toContain('读取失败')
    expect(wrapper.text()).not.toContain('正在读取设置')
  })
})
