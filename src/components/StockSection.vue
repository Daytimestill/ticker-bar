<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import {
  MAX_STOCKS,
  currencyName,
  currencyOptions,
  errorText,
  moveStock,
  pruneAlertsForStocks,
  removeStock,
  upsertStock,
  type AppConfig,
  type StockSearchResult,
} from '../settings'
import { useStockSearch } from '../hooks/useStockSearch'
import { usePointerDrag } from '../hooks/usePointerDrag'

const props = defineProps<{ config: AppConfig }>()
const emit = defineEmits<{
  'stock-selected': []
  'schedule-preview': []
}>()

const listError = ref('')

const {
  query,
  results,
  loading,
  open,
  error,
  activeIndex,
  schedule,
  select,
  handleKeydown,
  openSearch,
  closeSearch,
  dispose,
} = useStockSearch(addStock)

function addStock(result: StockSearchResult) {
  try {
    const { stocks, index } = upsertStock(props.config.stocks, result)
    props.config.stocks = stocks
    props.config.activeStock = index
    listError.value = ''
    emit('stock-selected')
  } catch (cause) {
    listError.value = errorText(cause)
  }
}

/** 列表顺序即菜单栏下拉的顺序，拖动或用箭头都能改。 */
function reorder(fromIndex: number, toIndex: number) {
  const result = moveStock(
    props.config.stocks,
    props.config.activeStock,
    fromIndex,
    toIndex,
  )
  props.config.stocks = result.stocks
  props.config.activeStock = result.activeStock
  listError.value = ''
  emit('stock-selected')
}

const {
  draggingIndex,
  dropTargetIndex,
  begin: beginPointerDrag,
  move: movePointerDrag,
  finish: finishPointerDrag,
  cancel: cancelPointerDrag,
  dispose: disposePointerDrag,
} = usePointerDrag(reorder)

function activate(index: number) {
  if (props.config.activeStock === index) return
  props.config.activeStock = index
  listError.value = ''
  emit('stock-selected')
}

function remove(index: number) {
  try {
    const stocks = removeStock(props.config.stocks, index)
    const removedActive = index === props.config.activeStock
    props.config.stocks = stocks
    // 引用被删股票的提醒规则一并清掉，否则会留下永远不触发的孤儿规则。
    props.config.alerts = pruneAlertsForStocks(props.config.alerts, stocks)
    if (removedActive) {
      props.config.activeStock = 0
    } else if (index < props.config.activeStock) {
      props.config.activeStock -= 1
    }
    listError.value = ''
    emit('stock-selected')
  } catch (cause) {
    listError.value = errorText(cause)
  }
}

const active = computed(
  () => props.config.stocks[props.config.activeStock] ?? null,
)

function marketOf(symbol: string): string {
  const upper = symbol.toUpperCase()
  if (upper.endsWith('.HK')) return '港股'
  if (upper.endsWith('.SH')) return '沪市'
  if (upper.endsWith('.SZ')) return '深市'
  if (upper.endsWith('.BJ')) return '北交所'
  return '待确认'
}

onBeforeUnmount(() => {
  dispose()
  disposePointerDrag()
})
</script>

<template>
  <section class="settings-page" data-testid="page-stock">
    <div class="page-toolbar">
      <span class="page-kicker">行情来源</span>
      <span class="status-pill">腾讯行情 · 无需密钥</span>
    </div>
    <div class="form-section">
      <div class="form-grid">
        <div class="stock-picker">
          <label for="stock-search">添加股票</label>
          <div class="stock-search-control">
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <circle cx="8.5" cy="8.5" r="5.25" />
              <path d="m12.4 12.4 3.3 3.3" />
            </svg>
            <input
              id="stock-search"
              v-model="query"
              data-testid="stock-search-input"
              role="combobox"
              aria-autocomplete="list"
              aria-controls="stock-search-listbox"
              :aria-expanded="open"
              :aria-activedescendant="
                activeIndex >= 0
                  ? `stock-search-result-${activeIndex}`
                  : undefined
              "
              autocomplete="off"
              maxlength="32"
              placeholder="输入股票名称或代码，例如：贵州茅台、600519"
              @input="schedule"
              @focus="openSearch"
              @blur="closeSearch"
              @keydown="handleKeydown"
            />
            <span v-if="loading" class="search-spinner" aria-label="正在搜索"></span>
            <span v-else class="search-shortcut">
              {{ config.stocks.length }}/{{ MAX_STOCKS }}
            </span>
          </div>

          <div
            v-if="open"
            id="stock-search-listbox"
            class="stock-search-popover"
            role="listbox"
          >
            <button
              v-for="(result, index) in results"
              :id="`stock-search-result-${index}`"
              :key="result.symbol"
              type="button"
              role="option"
              class="stock-search-option"
              :class="{ 'is-active': activeIndex === index }"
              :aria-selected="activeIndex === index"
              :data-testid="`stock-search-option-${result.symbol}`"
              @mouseenter="activeIndex = index"
              @mousedown.prevent
              @click="select(result)"
            >
              <span class="market-token">{{ result.market }}</span>
              <span class="stock-result-name">
                <strong>{{ result.name }}</strong>
                <small>{{ result.currency }} 行情</small>
              </span>
              <code>{{ result.symbol }}</code>
            </button>
            <div v-if="!loading && !results.length" class="stock-search-empty">
              <strong>{{ error ? '暂时无法搜索' : '没有找到匹配股票' }}</strong>
              <span>{{ error || '换一个名称或输入完整代码试试' }}</span>
            </div>
          </div>
        </div>

        <div class="stock-list" data-testid="stock-list">
          <div
            v-for="(stock, index) in config.stocks"
            :key="stock.symbol"
            class="stock-row"
            :class="{
              'is-active': index === config.activeStock,
              'is-dragging': draggingIndex === index,
              'is-drop-target': dropTargetIndex === index,
            }"
            :data-testid="`stock-row-${stock.symbol}`"
            :data-drag-index="index"
            role="button"
            tabindex="0"
            :aria-pressed="index === config.activeStock"
            @click="activate(index)"
            @keydown.enter.prevent="activate(index)"
          >
            <span
              v-if="config.stocks.length > 1"
              class="drag-handle"
              aria-hidden="true"
              :data-testid="`stock-drag-${stock.symbol}`"
              @click.stop
              @pointerdown.stop="beginPointerDrag(index, $event)"
              @pointermove="movePointerDrag"
              @pointerup="finishPointerDrag"
              @pointercancel="cancelPointerDrag"
            >
              ⠿
            </span>
            <span class="market-token">{{ marketOf(stock.symbol) }}</span>
            <span class="stock-result-name">
              <span class="stock-row-title">
                <strong>{{ stock.shortName || '未命名股票' }}</strong>
                <!-- 置顶标记跟着股票名走：放在操作列会让两种行宽度不一，把整列顶歪 -->
                <span
                  v-if="index === config.activeStock"
                  class="stock-row-badge"
                  data-testid="active-stock-badge"
                >
                  菜单栏显示
                </span>
              </span>
              <small>{{ currencyName(stock.currency) }}行情</small>
            </span>
            <code>{{ stock.symbol }}</code>
            <div v-if="config.stocks.length > 1" class="move-buttons">
              <button
                type="button"
                :aria-label="`上移 ${stock.shortName || stock.symbol}`"
                :data-testid="`stock-move-up-${stock.symbol}`"
                :disabled="index === 0"
                @click.stop="reorder(index, index - 1)"
              >
                ↑
              </button>
              <button
                type="button"
                :aria-label="`下移 ${stock.shortName || stock.symbol}`"
                :data-testid="`stock-move-down-${stock.symbol}`"
                :disabled="index === config.stocks.length - 1"
                @click.stop="reorder(index, index + 1)"
              >
                ↓
              </button>
            </div>
            <span class="stock-row-actions">
              <button
                v-if="index !== config.activeStock"
                type="button"
                class="stock-row-remove"
                :aria-label="`移除 ${stock.shortName || stock.symbol}`"
                :data-testid="`stock-remove-${stock.symbol}`"
                @click.stop="remove(index)"
              >
                移除
              </button>
            </span>
          </div>
          <p
            v-if="listError"
            class="error"
            role="alert"
            data-testid="stock-list-error"
          >
            {{ listError }}
          </p>
        </div>

        <template v-if="active">
          <label>
            <span>菜单栏简称</span>
            <input
              data-testid="stock-short-name"
              v-model.trim="active.shortName"
              maxlength="12"
              autocomplete="off"
              @input="emit('schedule-preview')"
            />
          </label>
          <label>
            <span>交易货币</span>
            <select
              data-testid="stock-currency"
              v-model="active.currency"
              @change="emit('schedule-preview')"
            >
              <option
                v-for="option in currencyOptions"
                :key="option.value"
                :value="option.value"
              >
                {{ option.label }}
              </option>
              <!-- 旧配置里可能存着列表之外的币种，保留原值避免被静默改写 -->
              <option
                v-if="!currencyOptions.some((item) => item.value === active.currency)"
                :value="active.currency"
              >
                {{ active.currency || '未设置' }}
              </option>
            </select>
            <small class="field-hint">
              添加股票时按市场自动识别，仅用于持仓收益的币种归类，通常无需修改
            </small>
          </label>
        </template>
      </div>
      <p class="section-note">
        点击列表中的股票可切换菜单栏显示；拖动左侧手柄或使用箭头可调整顺序，
        列表顺序即菜单栏下拉里的顺序。
        当前版本查询腾讯行情报价，不读取券商账户，也不会申请交易权限
      </p>
    </div>
  </section>
</template>
