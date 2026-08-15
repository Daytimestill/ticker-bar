export type ProviderKind = 'mock' | 'tencent' | 'longbridge'

export type DisplayMetric =
  | 'symbol'
  | 'shortName'
  | 'lastPrice'
  | 'dailyChange'
  | 'dailyChangePercent'
  | 'previousClose'
  | 'dayHigh'
  | 'dayLow'
  | 'positionProfit'
  | 'positionReturnPercent'
  | 'marketValue'
  | 'averageCost'
  | 'quantity'
  | 'profitPerShare'
  | 'marketStatus'
  | 'updatedTime'

export type CompactStyle = 'none' | 'western' | 'chinese'
export type DisplayPreset = 'price' | 'priceChange' | 'position' | 'custom'

export interface MetricConfig {
  metric: DisplayMetric
  precision: number
  showSign: boolean
  directionArrow: boolean
  compactStyle: CompactStyle
  label: string | null
}

export interface DisplayConfig {
  items: MetricConfig[]
  separator: string
  appendClosedStatus: boolean
  appendDelayedStatus: boolean
}

export interface PositionConfig {
  quantity: string
  averageCost: string
}

export interface StockConfig {
  symbol: string
  shortName: string
  currency: string
  position: PositionConfig | null
}

export type AlertMetric =
  | 'price'
  | 'changePercent'
  | 'positionProfit'
  | 'positionReturnPercent'
export type AlertComparator = 'above' | 'below'
export type AlertRepeat = 'dailyOnce' | 'once'

export interface AlertRule {
  id: string
  symbol: string
  metric: AlertMetric
  comparator: AlertComparator
  threshold: string
  repeat: AlertRepeat
  enabled: boolean
  silent: boolean
  customTitle: string | null
  customBody: string | null
  lastTriggeredDay: string | null
}

export interface AppConfig {
  schemaVersion: number
  provider: ProviderKind
  stocks: StockConfig[]
  activeStock: number
  alerts: AlertRule[]
  display: DisplayConfig
  launchAtLogin: boolean
  trayThrottleMs: number
}

// 与后端 MAX_STOCKS 保持一致。
export const MAX_STOCKS = 8
// 与后端 MAX_ALERTS 保持一致。
export const MAX_ALERTS = 20

export interface RefreshStatus {
  lastSuccessAt: string | null
  lastError: string | null
}

/**
 * 提醒规则的编辑草稿。与 AlertRule 的区别：没有 id/enabled/lastTriggeredDay
 * 这些运行期字段，且自定义文案用空串而非 null——输入框绑不了 null。
 */
export interface AlertDraft {
  symbol: string
  metric: AlertMetric
  comparator: AlertComparator
  threshold: string
  repeat: AlertRepeat
  silent: boolean
  customTitle: string
  customBody: string
}

/** 窗口内提醒 Toast。key 由前端自增生成——同一条规则可以连着触发多次 */
export interface AlertToast {
  key: number
  title: string
  body: string
}

// 提醒连着触发时不该把整个窗口糊满，只留最近几条
export const MAX_TOASTS = 4

/**
 * 追加一条 Toast，超量时丢最旧的。
 *
 * 挤掉的是旧的而不是新的：用户最关心刚发生的那条，
 * 而且旧的那条大概率已经看过了。
 */
export function pushToast(toasts: AlertToast[], next: AlertToast): AlertToast[] {
  return [...toasts, next].slice(-MAX_TOASTS)
}

export interface StockSearchResult {
  symbol: string
  name: string
  market: string
  currency: string
}

/**
 * 可选交易货币。当前行情源只覆盖沪深京（人民币）与港股（港币），
 * 添加股票时已按市场自动识别，这里只是留个纠错口子。
 */
export const currencyOptions: { value: string; name: string; label: string }[] =
  [
    { value: 'CNY', name: '人民币', label: '人民币 CNY' },
    { value: 'HKD', name: '港币', label: '港币 HKD' },
  ]

function currencyOptionOf(currency: string) {
  const code = currency.trim().toUpperCase()
  return currencyOptions.find((option) => option.value === code)
}

/** 「人民币 CNY」——用于下拉与合计分组标题，币种代码要露出来。 */
export function currencyLabel(currency: string): string {
  const code = currency.trim().toUpperCase()
  return currencyOptionOf(currency)?.label ?? code
}

/** 「人民币」——用于行内说明，旁边已有股票代码，不必再带币种代码。 */
export function currencyName(currency: string): string {
  const code = currency.trim().toUpperCase()
  return currencyOptionOf(currency)?.name ?? code
}

/** 组合汇总（后端 Decimal 序列化为字符串）。 */
export interface PortfolioTotal {
  currency: string
  marketValue: string
  costBasis: string
  unrealizedProfit: string
  returnPercent: string
}

export interface PositionRow extends PortfolioTotal {
  symbol: string
  shortName: string
}

export interface PortfolioSummary {
  rows: PositionRow[]
  totals: PortfolioTotal[]
  missingQuotes: number
}

/**
 * 单个显示项支持哪些调整。
 *
 * 不是所有数据都值得配置：市场状态、股票简称这类纯文本项没有小数位可言，
 * 价格、市值这类恒正的数值加「正负号/箭头」也毫无意义。把不适用的控件直接
 * 藏掉，比摆在那里让人以为能调要诚实。
 */
export interface MetricCapability {
  /** 小数位数 */
  precision: boolean
  /** 正负号 / 箭头（只对可能为负的指标有意义） */
  format: boolean
  /** 万/亿、K/M 缩写（只对可能达到大数量级的指标有意义） */
  compact: boolean
}

const TEXT_METRICS: readonly DisplayMetric[] = [
  'symbol',
  'shortName',
  'marketStatus',
  'updatedTime',
]
const SIGNED_METRICS: readonly DisplayMetric[] = [
  'dailyChange',
  'dailyChangePercent',
  'positionProfit',
  'positionReturnPercent',
  'profitPerShare',
]
const COMPACT_METRICS: readonly DisplayMetric[] = [
  'positionProfit',
  'marketValue',
  'quantity',
]

export function metricCapability(metric: DisplayMetric): MetricCapability {
  if (TEXT_METRICS.includes(metric)) {
    return { precision: false, format: false, compact: false }
  }
  return {
    precision: true,
    format: SIGNED_METRICS.includes(metric),
    compact: COMPACT_METRICS.includes(metric),
  }
}

export function hasAnyNumericOption(metric: DisplayMetric): boolean {
  const capability = metricCapability(metric)
  return capability.precision || capability.format || capability.compact
}

export interface MetricOption {
  metric: DisplayMetric
  label: string
  group: '行情' | '持仓' | '状态'
}

export const metricOptions: MetricOption[] = [
  { metric: 'shortName', label: '股票简称', group: '行情' },
  { metric: 'symbol', label: '股票代码', group: '行情' },
  { metric: 'lastPrice', label: '当前价格', group: '行情' },
  { metric: 'dailyChange', label: '今日涨跌额', group: '行情' },
  { metric: 'dailyChangePercent', label: '今日涨跌幅', group: '行情' },
  { metric: 'previousClose', label: '昨日收盘价', group: '行情' },
  { metric: 'dayHigh', label: '今日最高价', group: '行情' },
  { metric: 'dayLow', label: '今日最低价', group: '行情' },
  { metric: 'positionProfit', label: '持仓收益', group: '持仓' },
  {
    metric: 'positionReturnPercent',
    label: '持仓收益率',
    group: '持仓',
  },
  { metric: 'marketValue', label: '当前市值', group: '持仓' },
  { metric: 'averageCost', label: '平均成本', group: '持仓' },
  { metric: 'quantity', label: '持仓数量', group: '持仓' },
  { metric: 'profitPerShare', label: '每股盈利', group: '持仓' },
  { metric: 'marketStatus', label: '市场状态', group: '状态' },
  { metric: 'updatedTime', label: '更新时间', group: '状态' },
]

export function recommendedMetric(metric: DisplayMetric): MetricConfig {
  const percentage =
    metric === 'dailyChangePercent' || metric === 'positionReturnPercent'
  const signed =
    metric === 'dailyChange' ||
    metric === 'positionProfit' ||
    metric === 'profitPerShare'
  const compact =
    metric === 'positionProfit' || metric === 'marketValue'

  return {
    metric,
    precision:
      metric === 'quantity' || metric === 'positionProfit' ? 0 : 2,
    showSign: signed,
    directionArrow: percentage,
    compactStyle: compact ? 'chinese' : 'none',
    label: null,
  }
}

export function applyPreset(preset: DisplayPreset): MetricConfig[] {
  switch (preset) {
    case 'price':
      return [recommendedMetric('lastPrice')]
    case 'priceChange':
      return [
        recommendedMetric('lastPrice'),
        recommendedMetric('dailyChangePercent'),
      ]
    case 'position':
      return [
        recommendedMetric('positionProfit'),
        recommendedMetric('positionReturnPercent'),
      ]
    case 'custom':
      return []
  }
}

export function createDefaultConfig(): AppConfig {
  return {
    schemaVersion: 2,
    provider: 'tencent',
    // 示例股票只带行情不带持仓：首启不该展示编造的盈亏数据。
    stocks: [
      {
        symbol: '01810.HK',
        shortName: '小米',
        currency: 'HKD',
        position: null,
      },
    ],
    activeStock: 0,
    alerts: [],
    display: {
      items: applyPreset('priceChange'),
      separator: ' ',
      // 「·收/·延」默认不展示：清楚市场状态的用户不需要，设置页可开。
      appendClosedStatus: false,
      appendDelayedStatus: false,
    },
    launchAtLogin: false,
    trayThrottleMs: 3_000,
  }
}

export function activeStockOf(config: AppConfig): StockConfig | null {
  return config.stocks[config.activeStock] ?? null
}

/**
 * 把搜索结果加入股票列表：已存在则复用原条目，返回其下标；
 * 超出上限抛错，由调用方展示。
 */
export function upsertStock(
  stocks: StockConfig[],
  result: StockSearchResult,
): { stocks: StockConfig[]; index: number } {
  const existing = stocks.findIndex(
    (stock) => stock.symbol.toUpperCase() === result.symbol.toUpperCase(),
  )
  if (existing >= 0) {
    return { stocks, index: existing }
  }
  if (stocks.length >= MAX_STOCKS) {
    throw new Error(`最多支持 ${MAX_STOCKS} 只股票`)
  }
  return {
    stocks: [
      ...stocks,
      {
        symbol: result.symbol,
        shortName: result.name,
        currency: result.currency,
        position: null,
      },
    ],
    index: stocks.length,
  }
}

/**
 * 调整股票顺序，并让「置顶」始终跟着原来那只股票走。
 * activeStock 存的是下标，挪动后必须重新定位，否则菜单栏会静默换成别的股票。
 */
export function moveStock(
  stocks: StockConfig[],
  activeStock: number,
  fromIndex: number,
  toIndex: number,
): { stocks: StockConfig[]; activeStock: number } {
  if (
    fromIndex < 0 ||
    fromIndex >= stocks.length ||
    toIndex < 0 ||
    toIndex >= stocks.length ||
    fromIndex === toIndex
  ) {
    return { stocks, activeStock }
  }

  const pinned = stocks[activeStock] ?? null
  const next = [...stocks]
  const [moved] = next.splice(fromIndex, 1)
  next.splice(toIndex, 0, moved)
  const nextActive = pinned ? next.indexOf(pinned) : activeStock

  return { stocks: next, activeStock: nextActive < 0 ? 0 : nextActive }
}

export function removeStock(stocks: StockConfig[], index: number): StockConfig[] {
  if (index < 0 || index >= stocks.length) {
    return stocks
  }
  if (stocks.length === 1) {
    throw new Error('至少保留一只股票')
  }
  return stocks.filter((_, current) => current !== index)
}

export const alertMetricOptions: { value: AlertMetric; label: string }[] = [
  { value: 'price', label: '股价' },
  { value: 'changePercent', label: '今日涨跌幅' },
  { value: 'positionProfit', label: '持仓收益' },
  { value: 'positionReturnPercent', label: '持仓收益率' },
]

const SIGNED_DECIMAL_PATTERN = /^-?\d+(\.\d+)?$/

export function alertMetricLabel(metric: AlertMetric): string {
  return (
    alertMetricOptions.find((option) => option.value === metric)?.label ?? metric
  )
}

/** 提醒规则一句话摘要，用于规则列表展示。 */
export function describeAlert(rule: AlertRule, stocks: StockConfig[]): string {
  const stock = stocks.find(
    (candidate) => candidate.symbol.toUpperCase() === rule.symbol.toUpperCase(),
  )
  const name = stock?.shortName.trim() || rule.symbol
  const comparator = rule.comparator === 'above' ? '≥' : '≤'
  const percent =
    rule.metric === 'changePercent' || rule.metric === 'positionReturnPercent'
  return `${name} ${alertMetricLabel(rule.metric)} ${comparator} ${rule.threshold}${percent ? '%' : ''}`
}

/** 提醒规则草稿校验：阈值格式/范围、指标与持仓的依赖关系。 */
export function alertDraftError(
  rule: Pick<AlertRule, 'symbol' | 'metric' | 'threshold'>,
  stocks: StockConfig[],
): string | null {
  const stock = stocks.find(
    (candidate) => candidate.symbol.toUpperCase() === rule.symbol.toUpperCase(),
  )
  if (!stock) return '请选择股票'
  const threshold = rule.threshold.trim()
  if (!SIGNED_DECIMAL_PATTERN.test(threshold)) {
    return '阈值需要是数字（可为负数）'
  }
  if (Math.abs(Number(threshold)) > MAX_POSITION_VALUE) {
    return '阈值超出可支持的范围'
  }
  if (
    (rule.metric === 'positionProfit' ||
      rule.metric === 'positionReturnPercent') &&
    !stock.position
  ) {
    return `${stock.shortName.trim() || stock.symbol} 未启用持仓计算，无法使用持仓类指标`
  }
  return null
}

/**
 * 全量校验已保存的提醒规则，返回第一条错误。
 * 保存前的最后闸门：删股票、关持仓等操作可能让既有规则失效，
 * 只靠新建表单的草稿校验兜不住。
 */
export function firstAlertError(
  alerts: AlertRule[],
  stocks: StockConfig[],
): string | null {
  if (alerts.length > MAX_ALERTS) {
    return `最多支持 ${MAX_ALERTS} 条提醒规则`
  }
  for (const alert of alerts) {
    const error = alertDraftError(alert, stocks)
    if (error) {
      return `提醒规则「${describeAlert(alert, stocks)}」：${error}`
    }
  }
  return null
}

/** 删除股票后联动清理引用它的提醒规则，避免留下永远不触发的孤儿规则。 */
export function pruneAlertsForStocks(
  alerts: AlertRule[],
  stocks: StockConfig[],
): AlertRule[] {
  const symbols = new Set(stocks.map((stock) => stock.symbol.toUpperCase()))
  return alerts.filter((alert) => symbols.has(alert.symbol.toUpperCase()))
}

/** 全量校验各股票的持仓输入，返回第一条错误（带股票名前缀）。 */
export function firstPositionError(stocks: StockConfig[]): string | null {
  for (const stock of stocks) {
    const error = positionInputError(stock.position)
    if (error) {
      const name = stock.shortName.trim() || stock.symbol
      return `${name}：${error}`
    }
  }
  return null
}

export function toggleMetric(
  items: MetricConfig[],
  metric: DisplayMetric,
  selected: boolean,
): MetricConfig[] {
  const alreadySelected = items.some((item) => item.metric === metric)

  if (selected) {
    return alreadySelected ? items : [...items, recommendedMetric(metric)]
  }

  if (!alreadySelected) {
    return items
  }
  if (items.length === 1) {
    throw new Error('至少保留一个菜单栏数据项')
  }
  return items.filter((item) => item.metric !== metric)
}

export function errorText(error: unknown): string {
  if (typeof error === 'string') return error
  if (error instanceof Error) return error.message
  return String(error)
}

const DECIMAL_INPUT_PATTERN = /^\d+(\.\d+)?$/
// 与后端一致的数值上限（1 万亿），超出会被后端拒绝，前端提前拦截。
const MAX_POSITION_VALUE = 1_000_000_000_000

export function positionInputError(position: PositionConfig | null): string | null {
  // 两项都空 = 该股票不计算收益，是合法状态（由调用方置为 null）。
  if (!position) return null
  const fields = [
    { value: position.quantity.trim(), label: '持仓数量' },
    { value: position.averageCost.trim(), label: '平均成本' },
  ]
  if (fields.some((field) => !field.value)) {
    return '持仓数量与平均成本需要同时填写'
  }
  for (const field of fields) {
    if (!DECIMAL_INPUT_PATTERN.test(field.value)) {
      return `${field.label}需要是非负数字`
    }
    if (Number(field.value) > MAX_POSITION_VALUE) {
      return `${field.label}超出可支持的范围`
    }
  }
  return null
}

/** 单个输入框是否有问题，用于在出错的那一格上做标记而不是弹一整条红字。 */
export function positionFieldInvalid(
  position: PositionConfig | null,
  key: keyof PositionConfig,
): boolean {
  if (!position) return false
  const value = position[key].trim()
  if (!value) {
    // 两项都空是「不计算」，只有另一项填了才算这一格漏填。
    const other = key === 'quantity' ? position.averageCost : position.quantity
    return other.trim().length > 0
  }
  return (
    !DECIMAL_INPUT_PATTERN.test(value) || Number(value) > MAX_POSITION_VALUE
  )
}

/**
 * 按输入框内容组装持仓：两项都空表示不计算收益，返回 null。
 * 取代了原来的「启用持仓计算」开关——填了就是要算，不必再点一次开关。
 */
export function positionFromInput(
  quantity: string,
  averageCost: string,
): PositionConfig | null {
  if (!quantity.trim() && !averageCost.trim()) return null
  return { quantity, averageCost }
}

export function moveMetric(
  items: MetricConfig[],
  fromIndex: number,
  toIndex: number,
): MetricConfig[] {
  if (
    fromIndex < 0 ||
    fromIndex >= items.length ||
    toIndex < 0 ||
    toIndex >= items.length ||
    fromIndex === toIndex
  ) {
    return items
  }

  const next = [...items]
  const [moved] = next.splice(fromIndex, 1)
  next.splice(toIndex, 0, moved)
  return next
}
