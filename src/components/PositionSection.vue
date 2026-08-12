<script setup lang="ts">
import { computed } from 'vue'
import {
  currencyLabel,
  positionFieldInvalid,
  positionFromInput,
  positionInputError,
  type AppConfig,
  type PortfolioSummary,
  type PortfolioTotal,
  type PositionRow,
  type StockConfig,
} from '../settings'

const props = defineProps<{
  config: AppConfig
  summary: PortfolioSummary | null
}>()
const emit = defineEmits<{
  'schedule-preview': []
}>()

/** 逐行校验：错误就地显示在那一行，不在页面底部堆一条红色横幅。 */
function rowError(stock: StockConfig): string | null {
  return positionInputError(stock.position)
}

const rowsBySymbol = computed(() => {
  const map = new Map<string, PositionRow>()
  for (const row of props.summary?.rows ?? []) {
    map.set(row.symbol, row)
  }
  return map
})

const totals = computed<PortfolioTotal[]>(() => props.summary?.totals ?? [])

/** 已填写持仓的股票数，以及其中真正算进合计的数量——两者不等时要说明差在哪。 */
const configuredCount = computed(
  () => props.config.stocks.filter((stock) => stock.position !== null).length,
)
const countedCount = computed(() => props.summary?.rows.length ?? 0)

/** 按需拼接说明：句子之间用句号分隔，末句不带——省得条件分支各自处理标点。 */
const totalExplanation = computed(() => {
  const parts = [
    '收益 = Σ（现价 − 成本）× 数量，收益率 = 合计收益 ÷ 合计成本（按仓位加权，不是各股收益率的平均）',
  ]
  if (totals.value.length > 1) {
    parts.push('不同币种分开统计，不做汇率换算')
  }
  const missing = props.summary?.missingQuotes ?? 0
  if (missing > 0) {
    parts.push(`另有 ${missing} 只股票暂无行情，未计入`)
  }
  return parts.join('。')
})

function setPositionField(
  stock: StockConfig,
  field: 'quantity' | 'averageCost',
  value: string,
) {
  const current = stock.position ?? { quantity: '', averageCost: '' }
  const next = { ...current, [field]: value }
  stock.position = positionFromInput(next.quantity, next.averageCost)
  emit('schedule-preview')
}

/** 后端 Decimal 以字符串回传，这里统一格式化成带符号的两位小数。 */
function signedAmount(value: string): string {
  const amount = Number(value)
  if (!Number.isFinite(amount)) return '—'
  const sign = amount > 0 ? '+' : ''
  return `${sign}${amount.toFixed(2)}`
}

function signedPercent(value: string): string {
  const percent = Number(value)
  if (!Number.isFinite(percent)) return '—'
  if (percent === 0) return '0.00%'
  return `${percent > 0 ? '↑' : '↓'}${Math.abs(percent).toFixed(2)}%`
}

function toneOf(value: string): 'up' | 'down' | 'flat' {
  const amount = Number(value)
  if (!Number.isFinite(amount) || amount === 0) return 'flat'
  return amount > 0 ? 'up' : 'down'
}
</script>

<template>
  <section class="settings-page" data-testid="page-position">
    <p class="section-note">
      填写数量与平均成本即开始计算该股票的收益，两项都留空表示不计算。
      所有持仓数据仅保存在本机，不读取券商资产
    </p>

    <div class="position-list" data-testid="position-list">
      <div
        v-for="stock in config.stocks"
        :key="stock.symbol"
        class="position-row"
        :class="{ 'has-error': rowError(stock) !== null }"
        :data-testid="`position-row-${stock.symbol}`"
      >
        <div class="position-identity">
          <strong>{{ stock.shortName || stock.symbol }}</strong>
          <code>{{ stock.symbol }}</code>
        </div>
        <label>
          <span>持仓数量</span>
          <input
            :data-testid="`position-quantity-${stock.symbol}`"
            :class="{
              'is-invalid': positionFieldInvalid(stock.position, 'quantity'),
            }"
            :value="stock.position?.quantity ?? ''"
            inputmode="decimal"
            placeholder="留空不计算"
            @input="
              setPositionField(
                stock,
                'quantity',
                ($event.target as HTMLInputElement).value.trim(),
              )
            "
          />
        </label>
        <label>
          <span>平均成本</span>
          <input
            :data-testid="`position-average-cost-${stock.symbol}`"
            :class="{
              'is-invalid': positionFieldInvalid(stock.position, 'averageCost'),
            }"
            :value="stock.position?.averageCost ?? ''"
            inputmode="decimal"
            placeholder="留空不计算"
            @input="
              setPositionField(
                stock,
                'averageCost',
                ($event.target as HTMLInputElement).value.trim(),
              )
            "
          />
        </label>
        <div
          class="position-result"
          :data-testid="`position-result-${stock.symbol}`"
        >
          <small
            v-if="rowError(stock)"
            class="position-result-hint"
            role="alert"
            :data-testid="`position-error-${stock.symbol}`"
          >
            {{ rowError(stock) }}
          </small>
          <template v-else-if="rowsBySymbol.get(stock.symbol)">
            <strong
              :class="`tone-${toneOf(rowsBySymbol.get(stock.symbol)!.unrealizedProfit)}`"
            >
              {{ signedAmount(rowsBySymbol.get(stock.symbol)!.unrealizedProfit) }}
            </strong>
            <small>
              {{ signedPercent(rowsBySymbol.get(stock.symbol)!.returnPercent) }}
              · 市值
              {{ Number(rowsBySymbol.get(stock.symbol)!.marketValue).toFixed(2) }}
            </small>
          </template>
          <small v-else-if="stock.position">行情加载中…</small>
          <small v-else class="position-result-idle">未计算</small>
        </div>
      </div>
    </div>

    <div
      v-if="configuredCount > 0 || totals.length > 0"
      class="position-total"
      data-testid="position-total"
    >
      <div class="position-total-heading">
        <strong>合计</strong>
        <small>
          已计入 {{ countedCount }} / {{ configuredCount }} 只已填持仓的股票
        </small>
      </div>

      <div v-if="totals.length" class="position-total-grid">
        <div
          v-for="total in totals"
          :key="total.currency"
          class="position-total-item"
          :data-testid="`position-total-${total.currency}`"
        >
          <span>{{ currencyLabel(total.currency) }}</span>
          <strong :class="`tone-${toneOf(total.unrealizedProfit)}`">
            {{ signedAmount(total.unrealizedProfit) }}
          </strong>
          <small>
            收益率 {{ signedPercent(total.returnPercent) }}
          </small>
          <small>
            市值 {{ Number(total.marketValue).toFixed(2) }} · 成本
            {{ Number(total.costBasis).toFixed(2) }}
          </small>
        </div>
      </div>
      <p v-else class="section-note" data-testid="position-total-pending">
        行情加载完成后显示合计
      </p>

      <p class="position-total-formula">{{ totalExplanation }}</p>
    </div>
  </section>
</template>
