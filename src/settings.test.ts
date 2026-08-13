import { describe, expect, it } from 'vitest'
import {
  MAX_ALERTS,
  MAX_STOCKS,
  activeStockOf,
  alertDraftError,
  applyPreset,
  createDefaultConfig,
  describeAlert,
  errorText,
  firstAlertError,
  firstPositionError,
  hasAnyNumericOption,
  metricCapability,
  moveMetric,
  moveStock,
  positionFromInput,
  positionInputError,
  pruneAlertsForStocks,
  pushToast,
  MAX_TOASTS,
  removeStock,
  toggleMetric,
  upsertStock,
  type AlertRule,
} from './settings'

const INSPUR = {
  symbol: '600519.SH',
  name: '贵州茅台',
  market: '沪市',
  currency: 'CNY',
}

describe('settings model', () => {
  it('starts with price and daily percentage selected', () => {
    const config = createDefaultConfig()

    expect(config.provider).toBe('tencent')
    expect(config.trayThrottleMs).toBe(3_000)
    expect(config.display.items.map((item) => item.metric)).toEqual([
      'lastPrice',
      'dailyChangePercent',
    ])
  })

  it('adds a metric with its recommended formatting', () => {
    const config = createDefaultConfig()

    config.display.items = toggleMetric(
      config.display.items,
      'positionProfit',
      true,
    )

    expect(config.display.items.at(-1)).toMatchObject({
      metric: 'positionProfit',
      precision: 0,
      showSign: true,
      directionArrow: false,
      compactStyle: 'chinese',
    })
  })

  it('removes a metric without changing the order of remaining fields', () => {
    const config = createDefaultConfig()
    config.display.items = toggleMetric(
      config.display.items,
      'positionProfit',
      true,
    )

    config.display.items = toggleMetric(
      config.display.items,
      'dailyChangePercent',
      false,
    )

    expect(config.display.items.map((item) => item.metric)).toEqual([
      'lastPrice',
      'positionProfit',
    ])
  })

  it('moves selected metrics while preserving their formatting', () => {
    const config = createDefaultConfig()
    const original = config.display.items[1]

    config.display.items = moveMetric(config.display.items, 1, 0)

    expect(config.display.items[0]).toEqual(original)
    expect(config.display.items.map((item) => item.metric)).toEqual([
      'dailyChangePercent',
      'lastPrice',
    ])
  })

  it('applies the position preset', () => {
    const items = applyPreset('position')

    expect(items.map((item) => item.metric)).toEqual([
      'positionProfit',
      'positionReturnPercent',
    ])
  })

  it('does not allow removing the final visible metric', () => {
    const config = createDefaultConfig()
    config.display.items = applyPreset('price')

    expect(() =>
      toggleMetric(config.display.items, 'lastPrice', false),
    ).toThrowError('至少保留一个菜单栏数据项')
  })

  it('keeps toggling a missing metric off as a no-op', () => {
    const config = createDefaultConfig()

    const unchanged = toggleMetric(config.display.items, 'positionProfit', false)

    expect(unchanged).toBe(config.display.items)
  })

  it('ignores out-of-range metric moves', () => {
    const config = createDefaultConfig()

    expect(moveMetric(config.display.items, 0, 5)).toBe(config.display.items)
    expect(moveMetric(config.display.items, -1, 0)).toBe(config.display.items)
    expect(moveMetric(config.display.items, 1, 1)).toBe(config.display.items)
  })

  it('validates position inputs before they reach the backend', () => {
    expect(positionInputError(null)).toBeNull()
    expect(positionInputError({ quantity: '250', averageCost: '39.46' })).toBeNull()
    // 只填一半是「填漏了」而不是「不计算」，要与两项全空区分开
    expect(positionInputError({ quantity: '', averageCost: '39.46' })).toBe(
      '持仓数量与平均成本需要同时填写',
    )
    expect(positionInputError({ quantity: '250', averageCost: '  ' })).toBe(
      '持仓数量与平均成本需要同时填写',
    )
    expect(positionInputError({ quantity: '250', averageCost: 'abc' })).toBe(
      '平均成本需要是非负数字',
    )
    expect(positionInputError({ quantity: '-1', averageCost: '39.46' })).toBe(
      '持仓数量需要是非负数字',
    )
    expect(
      positionInputError({ quantity: '2000000000000', averageCost: '1' }),
    ).toBe('持仓数量超出可支持的范围')
  })

  it('adds searched stocks to the list and reuses existing entries', () => {
    const config = createDefaultConfig()

    const added = upsertStock(config.stocks, INSPUR)
    expect(added.index).toBe(1)
    expect(added.stocks).toHaveLength(2)
    expect(added.stocks[1]).toMatchObject({
      symbol: '600519.SH',
      shortName: '贵州茅台',
      currency: 'CNY',
      position: null,
    })

    const reused = upsertStock(added.stocks, INSPUR)
    expect(reused.index).toBe(1)
    expect(reused.stocks).toBe(added.stocks)
  })

  it('caps the stock list at the shared maximum', () => {
    const config = createDefaultConfig()
    let stocks = config.stocks
    for (let index = 1; index < MAX_STOCKS; index += 1) {
      stocks = upsertStock(stocks, {
        ...INSPUR,
        symbol: `60000${index}.SH`,
      }).stocks
    }

    expect(stocks).toHaveLength(MAX_STOCKS)
    expect(() => upsertStock(stocks, INSPUR)).toThrowError(
      `最多支持 ${MAX_STOCKS} 只股票`,
    )
  })

  it('调整股票顺序时置顶跟着原来那只股票走', () => {
    const config = createDefaultConfig()
    const { stocks } = upsertStock(config.stocks, INSPUR)
    const withThird = upsertStock(stocks, {
      symbol: '000001.SZ',
      name: '平安银行',
      market: '深市',
      currency: 'CNY',
    }).stocks
    // [小米, 贵州茅台, 平安银行]，置顶贵州茅台（下标 1）

    // 把平安银行挪到最前：置顶的贵州茅台下标顺移到 2，但仍是同一只
    const moved = moveStock(withThird, 1, 2, 0)
    expect(moved.stocks.map((stock) => stock.symbol)).toEqual([
      '000001.SZ',
      '01810.HK',
      '600519.SH',
    ])
    expect(moved.stocks[moved.activeStock].symbol).toBe('600519.SH')

    // 拖动置顶项自身，置顶依旧跟随
    const movedActive = moveStock(withThird, 1, 1, 0)
    expect(movedActive.stocks[movedActive.activeStock].symbol).toBe('600519.SH')
    expect(movedActive.activeStock).toBe(0)

    // 越界与原地移动一律原样返回
    expect(moveStock(withThird, 1, 0, 0).stocks).toBe(withThird)
    expect(moveStock(withThird, 1, -1, 2).stocks).toBe(withThird)
    expect(moveStock(withThird, 1, 0, 9).stocks).toBe(withThird)
  })

  it('removes stocks but never the final one', () => {
    const config = createDefaultConfig()
    const two = upsertStock(config.stocks, INSPUR).stocks

    const remaining = removeStock(two, 0)
    expect(remaining).toHaveLength(1)
    expect(remaining[0].symbol).toBe('600519.SH')

    expect(() => removeStock(remaining, 0)).toThrowError('至少保留一只股票')
    expect(removeStock(two, 5)).toBe(two)
  })

  it('reports the active stock and per-stock position errors', () => {
    const config = createDefaultConfig()
    expect(activeStockOf(config)?.symbol).toBe('01810.HK')

    expect(firstPositionError(config.stocks)).toBeNull()
    const broken = upsertStock(config.stocks, INSPUR).stocks.map((stock) =>
      stock.symbol === '600519.SH'
        ? { ...stock, position: { quantity: 'abc', averageCost: '1' } }
        : stock,
    )
    expect(firstPositionError(broken)).toBe('贵州茅台：持仓数量需要是非负数字')
  })

  it('describes alert rules with stock name and percent suffix', () => {
    const config = createDefaultConfig()
    const rule: AlertRule = {
      id: 'r1',
      symbol: '01810.HK',
      metric: 'changePercent',
      comparator: 'above',
      threshold: '3',
      repeat: 'dailyOnce',
      enabled: true,
      silent: false,
      customTitle: null,
      customBody: null,
      lastTriggeredDay: null,
    }
    expect(describeAlert(rule, config.stocks)).toBe('小米 今日涨跌幅 ≥ 3%')
    expect(
      describeAlert(
        { ...rule, metric: 'positionProfit', comparator: 'below', threshold: '-2000' },
        config.stocks,
      ),
    ).toBe('小米 持仓收益 ≤ -2000')
  })

  it('validates alert drafts including position-metric dependencies', () => {
    const config = createDefaultConfig()
    const base = { symbol: '01810.HK', metric: 'price' as const, threshold: '30' }

    expect(alertDraftError(base, config.stocks)).toBeNull()
    expect(
      alertDraftError({ ...base, threshold: '-3.5' }, config.stocks),
    ).toBeNull()
    expect(alertDraftError({ ...base, threshold: 'abc' }, config.stocks)).toBe(
      '阈值需要是数字（可为负数）',
    )
    expect(alertDraftError({ ...base, symbol: '999999.SH' }, config.stocks)).toBe(
      '请选择股票',
    )

    const noPosition = config.stocks.map((stock) => ({ ...stock, position: null }))
    expect(
      alertDraftError({ ...base, metric: 'positionProfit' }, noPosition),
    ).toBe('小米 未启用持仓计算，无法使用持仓类指标')
  })

  it('re-validates saved alerts so stale rules cannot be persisted', () => {
    const config = createDefaultConfig()
    const rule: AlertRule = {
      id: 'r1',
      symbol: '01810.HK',
      metric: 'price',
      comparator: 'above',
      threshold: '30',
      repeat: 'dailyOnce',
      enabled: true,
      silent: false,
      customTitle: null,
      customBody: null,
      lastTriggeredDay: null,
    }

    expect(firstAlertError([rule], config.stocks)).toBeNull()

    // 关掉持仓后，依赖持仓的规则变成死规则，保存前必须拦下
    const positionRule: AlertRule = { ...rule, metric: 'positionProfit' }
    expect(firstAlertError([positionRule], config.stocks)).toContain(
      '未启用持仓计算',
    )

    // 数量上限与后端一致
    const flood = Array.from({ length: MAX_ALERTS + 1 }, (_, index) => ({
      ...rule,
      id: `r${index}`,
    }))
    expect(firstAlertError(flood, config.stocks)).toBe(
      `最多支持 ${MAX_ALERTS} 条提醒规则`,
    )
  })

  it('prunes alerts that reference removed stocks', () => {
    const config = createDefaultConfig()
    const { stocks } = upsertStock(config.stocks, INSPUR)
    const keep: AlertRule = {
      id: 'keep',
      symbol: '01810.HK',
      metric: 'price',
      comparator: 'above',
      threshold: '30',
      repeat: 'dailyOnce',
      enabled: true,
      silent: false,
      customTitle: null,
      customBody: null,
      lastTriggeredDay: null,
    }
    const orphan: AlertRule = { ...keep, id: 'orphan', symbol: '600519.SH' }

    const remaining = pruneAlertsForStocks(
      [keep, orphan],
      removeStock(stocks, 1),
    )

    expect(remaining).toEqual([keep])
  })

  it('只为真正适用的指标暴露数值调整能力', () => {
    // 纯文本项：三项数值调整全都不适用
    for (const metric of [
      'marketStatus',
      'shortName',
      'symbol',
      'updatedTime',
    ] as const) {
      expect(metricCapability(metric)).toEqual({
        precision: false,
        format: false,
        compact: false,
      })
      expect(hasAnyNumericOption(metric)).toBe(false)
    }

    // 恒为正的价格：能调小数位，正负号/箭头无意义
    expect(metricCapability('lastPrice')).toEqual({
      precision: true,
      format: false,
      compact: false,
    })

    // 涨跌幅可正可负但不会到万级
    expect(metricCapability('dailyChangePercent')).toEqual({
      precision: true,
      format: true,
      compact: false,
    })

    // 持仓收益：可正可负且可达万级，三项全适用
    expect(metricCapability('positionProfit')).toEqual({
      precision: true,
      format: true,
      compact: true,
    })

    // 市值恒为正但数额大：只有缩写有意义
    expect(metricCapability('marketValue')).toEqual({
      precision: true,
      format: false,
      compact: true,
    })
  })

  it('两项都空视为不计算收益，填任意一项即建立持仓', () => {
    expect(positionFromInput('', '')).toBeNull()
    expect(positionFromInput('  ', ' ')).toBeNull()
    expect(positionFromInput('100', '')).toEqual({
      quantity: '100',
      averageCost: '',
    })
    expect(positionFromInput('100', '9.9')).toEqual({
      quantity: '100',
      averageCost: '9.9',
    })
  })

  it('drops the oldest toast once the stack is full, never the newest', () => {
    const filled = Array.from({ length: MAX_TOASTS }, (_, index) => ({
      key: index + 1,
      title: `提醒 ${index + 1}`,
      body: '',
    }))

    const next = pushToast(filled, { key: 99, title: '最新提醒', body: '' })

    expect(next).toHaveLength(MAX_TOASTS)
    expect(next.at(-1)?.title).toBe('最新提醒')
    expect(next.some((toast) => toast.key === 1)).toBe(false)
  })

  it('turns thrown values into user-facing text without an Error: prefix', () => {
    expect(errorText('后端返回的错误')).toBe('后端返回的错误')
    expect(errorText(new Error('至少保留一个菜单栏数据项'))).toBe(
      '至少保留一个菜单栏数据项',
    )
    expect(errorText(42)).toBe('42')
  })
})
