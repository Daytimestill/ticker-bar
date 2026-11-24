<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  currencyName,
  errorText,
  quickViewShortcutLabel,
  type ConnectionState,
  type QuickViewSnapshot,
} from '../settings'

const snapshot = ref<QuickViewSnapshot | null>(null)
const loading = ref(true)
const loadError = ref('')
let refreshTimer: ReturnType<typeof setInterval> | null = null

const statusText = computed(() => {
  const state = snapshot.value?.connection
  const labels: Record<ConnectionState, string> = {
    connecting: '行情连接中',
    live: '行情正常',
    delayed: '行情延迟',
    closed: '市场休市',
    reconnecting: '正在重连',
    disconnected: '网络已断开',
    authenticationFailed: '行情异常',
  }
  return state ? labels[state] : '读取行情中'
})

const updateText = computed(() => {
  if (snapshot.value?.updatedAt) return snapshot.value.updatedAt
  if (snapshot.value?.lastSuccessAt) return snapshot.value.lastSuccessAt
  return '尚未更新'
})

function numberOf(value: string | null): number | null {
  if (value === null) return null
  const number = Number(value)
  return Number.isFinite(number) ? number : null
}

function amount(value: string | null): string {
  const number = numberOf(value)
  if (number === null) return '—'
  const sign = number > 0 ? '+' : ''
  return `${sign}${number.toLocaleString('zh-CN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`
}

function price(value: string | null): string {
  const number = numberOf(value)
  if (number === null) return '—'
  return number.toLocaleString('zh-CN', {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })
}

function percent(value: string | null): string {
  const number = numberOf(value)
  if (number === null) return '—'
  if (number === 0) return '0.00%'
  return `${number > 0 ? '↑' : '↓'}${Math.abs(number).toFixed(2)}%`
}

function tone(value: string | null): 'up' | 'down' | 'flat' {
  const number = numberOf(value)
  if (number === null || number === 0) return 'flat'
  return number > 0 ? 'up' : 'down'
}

function marketOf(symbol: string): string {
  return symbol.split('.').at(-1)?.toUpperCase() ?? ''
}

async function loadSnapshot() {
  try {
    snapshot.value = await invoke<QuickViewSnapshot>('get_quick_view_snapshot')
    loadError.value = ''
  } catch (error) {
    loadError.value = errorText(error)
  } finally {
    loading.value = false
  }
}

function close() {
  void invoke('hide_quick_view').catch(() => {})
}

function openSettings() {
  void invoke('open_settings').catch(() => {})
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') close()
}

function handleVisibility() {
  if (document.hidden) {
    if (refreshTimer) clearInterval(refreshTimer)
    refreshTimer = null
    return
  }
  void loadSnapshot()
  if (!refreshTimer) {
    refreshTimer = setInterval(() => void loadSnapshot(), 3_000)
  }
}

onMounted(() => {
  void loadSnapshot()
  refreshTimer = setInterval(() => void loadSnapshot(), 3_000)
  window.addEventListener('keydown', handleKeydown)
  document.addEventListener('visibilitychange', handleVisibility)
})

onBeforeUnmount(() => {
  if (refreshTimer) clearInterval(refreshTimer)
  refreshTimer = null
  window.removeEventListener('keydown', handleKeydown)
  document.removeEventListener('visibilitychange', handleVisibility)
})
</script>

<template>
  <main class="quick-shell">
    <section class="quick-panel" aria-label="TickerBar 实时速览">
      <header class="quick-head">
        <div class="quick-brand">
          <span class="quick-logo" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="m4 17 5-6 4 3 7-9" />
              <path d="M16 5h4v4" />
            </svg>
          </span>
          <span>
            <strong>TickerBar</strong>
            <small>
              <i :class="`state-${snapshot?.connection ?? 'connecting'}`"></i>
              {{ statusText }} · {{ updateText }}
            </small>
          </span>
        </div>
        <button
          type="button"
          class="quick-icon-button"
          data-testid="quick-open-settings"
          aria-label="打开设置"
          title="打开设置"
          @click="openSettings"
        >
          <svg viewBox="0 0 18 18" aria-hidden="true">
            <circle cx="4" cy="9" r="1.35" />
            <circle cx="9" cy="9" r="1.35" />
            <circle cx="14" cy="9" r="1.35" />
          </svg>
        </button>
      </header>

      <div v-if="loading && !snapshot" class="quick-state">正在读取实时行情…</div>
      <div v-else-if="loadError && !snapshot" class="quick-state" data-testid="quick-error">
        <strong>暂时无法读取行情</strong>
        <small>{{ loadError }}</small>
        <button data-testid="quick-retry" type="button" @click="loadSnapshot">重试</button>
      </div>
      <template v-else-if="snapshot">
        <div v-if="snapshot.totals.length" class="quick-totals">
          <article
            v-for="total in snapshot.totals"
            :key="total.currency"
            class="quick-total"
            :data-testid="`quick-total-${total.currency}`"
          >
            <div class="quick-total-label">
              <span>总持仓收益</span>
              <span>{{ currencyName(total.currency) }}账户</span>
            </div>
            <div class="quick-total-value">
              <strong :class="`tone-${tone(total.unrealizedProfit)}`">
                {{ amount(total.unrealizedProfit) }}
              </strong>
              <em :class="`tone-${tone(total.returnPercent)}`">
                {{ percent(total.returnPercent) }}
              </em>
            </div>
          </article>
        </div>
        <div v-else class="quick-empty-total">
          <span>总持仓收益</span>
          <strong>暂无持仓数据</strong>
        </div>

        <div class="quick-table-head" aria-hidden="true">
          <span>股票</span><span>现价</span><span>今日</span><span>持仓收益</span>
        </div>
        <div class="quick-list">
          <article
            v-for="row in snapshot.rows"
            :key="row.symbol"
            class="quick-row"
            :data-testid="`quick-row-${row.symbol}`"
          >
            <div class="quick-identity">
              <strong>{{ row.shortName }}</strong>
              <small>{{ row.symbol.split('.')[0] }} · {{ marketOf(row.symbol) }}</small>
            </div>
            <strong class="quick-number">{{ price(row.lastPrice) }}</strong>
            <strong :class="`quick-number tone-${tone(row.dailyChangePercent)}`">
              {{ percent(row.dailyChangePercent) }}
            </strong>
            <strong :class="`quick-number tone-${tone(row.positionProfit)}`">
              {{ amount(row.positionProfit) }}
            </strong>
          </article>
        </div>

        <p v-if="snapshot.lastError" class="quick-warning">
          上次更新异常，当前显示最近一次成功行情
        </p>
        <p v-else-if="snapshot.missingQuotes" class="quick-warning">
          {{ snapshot.missingQuotes }} 只持仓暂无行情，未计入合计
        </p>
      </template>

      <footer class="quick-foot">
        <span>实时数据来自当前行情缓存</span>
        <span><kbd>{{ quickViewShortcutLabel(snapshot?.shortcut ?? 'Control+Command+T') }}</kbd> 收起 <kbd>esc</kbd></span>
      </footer>
    </section>
  </main>
</template>

<style scoped>
.quick-shell {
  --quick-panel: rgb(27 29 34 / 96%);
  --quick-line: rgb(255 255 255 / 9%);
  --quick-muted: rgb(242 245 248 / 51%);
  --quick-text: #f5f7f9;
  --quick-up: #ff666d;
  --quick-down: #40d39c;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
  /* 只留 1px 安全边，保证圆角描边完整落在透明窗口内。 */
  padding: 1px;
  color: var(--quick-text);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC", sans-serif;
}

.quick-panel {
  display: flex;
  width: 100%;
  height: 100%;
  flex-direction: column;
  overflow: hidden;
  border: 1px solid rgb(255 255 255 / 16%);
  border-radius: 21px;
  padding: 17px;
  background:
    linear-gradient(145deg, rgb(255 255 255 / 6%), transparent 45%),
    var(--quick-panel);
  /* 外阴影会被透明 WebView 的矩形边界裁切成假边框，只保留内高光。 */
  box-shadow: inset 0 1px rgb(255 255 255 / 7%);
  backdrop-filter: blur(34px) saturate(145%);
}

.quick-head,
.quick-brand,
.quick-brand > span,
.quick-total-label,
.quick-total-value,
.quick-foot {
  display: flex;
  align-items: center;
}

.quick-head { justify-content: space-between; padding: 0 1px 14px; }
.quick-brand { gap: 10px; }
.quick-brand > span:last-child { align-items: flex-start; flex-direction: column; }
.quick-brand strong { font-size: 14px; line-height: 1.2; }
.quick-brand small { display: flex; align-items: center; gap: 5px; margin-top: 3px; color: var(--quick-muted); font-size: 10px; }
.quick-brand i { width: 6px; height: 6px; border-radius: 50%; background: #55dc9b; box-shadow: 0 0 0 4px rgb(85 220 155 / 10%); }
.quick-brand i.state-disconnected,
.quick-brand i.state-authenticationFailed { background: var(--quick-up); box-shadow: 0 0 0 4px rgb(255 102 109 / 10%); }
.quick-brand i.state-delayed,
.quick-brand i.state-closed { background: #f6bf62; box-shadow: 0 0 0 4px rgb(246 191 98 / 10%); }

.quick-logo {
  display: grid;
  width: 29px;
  height: 29px;
  place-items: center;
  border-radius: 9px;
  color: #0a183a;
  background: linear-gradient(145deg, #a8c3ff, #5e84e8);
  box-shadow: inset 0 1px rgb(255 255 255 / 45%), 0 4px 12px rgb(77 111 206 / 32%);
}
.quick-logo svg { width: 17px; height: 17px; fill: none; stroke: currentColor; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
.quick-icon-button { display: grid; width: 30px; height: 30px; place-items: center; border: 0; border-radius: 9px; padding: 0; color: rgb(255 255 255 / 66%); background: rgb(255 255 255 / 6%); }
.quick-icon-button svg { width: 18px; height: 18px; fill: currentColor; }
.quick-icon-button:hover { color: white; background: rgb(255 255 255 / 10%); }

.quick-totals { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 8px; }
.quick-total,
.quick-empty-total { border: 1px solid rgb(255 255 255 / 7%); border-radius: 15px; padding: 15px 17px; background: radial-gradient(circle at 85% 20%, rgb(255 103 112 / 11%), transparent 39%), rgb(255 255 255 / 4%); }
.quick-total-label { justify-content: space-between; color: var(--quick-muted); font-size: 10px; }
.quick-total-value { align-items: baseline; gap: 10px; margin-top: 7px; }
.quick-total-value strong { font-size: 26px; letter-spacing: -.55px; line-height: 1.1; }
.quick-total-value em { font-size: 11px; font-style: normal; font-weight: 700; }
.quick-empty-total { display: flex; align-items: center; justify-content: space-between; color: var(--quick-muted); font-size: 11px; }
.quick-empty-total strong { color: rgb(255 255 255 / 58%); font-size: 13px; }

.quick-table-head,
.quick-row { display: grid; grid-template-columns: minmax(0, 1fr) 70px 72px 88px; column-gap: 7px; align-items: center; }
.quick-table-head { padding: 14px 11px 7px; color: rgb(255 255 255 / 32%); font-size: 9px; }
.quick-table-head span:not(:first-child) { text-align: right; }
.quick-list { min-height: 0; overflow: auto; border: 1px solid rgb(255 255 255 / 7%); border-radius: 14px; background: rgb(255 255 255 / 2.5%); scrollbar-width: thin; scrollbar-color: rgb(255 255 255 / 16%) transparent; }
.quick-row { min-height: 57px; border-bottom: 1px solid var(--quick-line); padding: 9px 11px; }
.quick-row:last-child { border-bottom: 0; }
.quick-identity { min-width: 0; }
.quick-identity strong,
.quick-identity small { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.quick-identity strong { font-size: 12px; }
.quick-identity small { margin-top: 3px; color: rgb(255 255 255 / 34%); font-size: 9px; letter-spacing: .2px; }
.quick-number { text-align: right; font-size: 11px; font-variant-numeric: tabular-nums; }
.tone-up { color: var(--quick-up); }
.tone-down { color: var(--quick-down); }
.tone-flat { color: rgb(255 255 255 / 66%); }

.quick-state { display: grid; min-height: 270px; place-content: center; justify-items: center; gap: 8px; color: var(--quick-muted); text-align: center; }
.quick-state strong { color: var(--quick-text); font-size: 14px; }
.quick-state small { max-width: 300px; line-height: 1.5; }
.quick-state button { border: 1px solid rgb(255 255 255 / 10%); border-radius: 8px; padding: 6px 14px; color: var(--quick-text); background: rgb(255 255 255 / 7%); }
.quick-warning { margin: 9px 3px 0; color: #f1bf6c; font-size: 9px; }
.quick-foot { justify-content: space-between; margin-top: auto; padding: 12px 3px 0; color: rgb(255 255 255 / 31%); font-size: 9px; }
.quick-foot > span:last-child { display: flex; align-items: center; gap: 3px; }
.quick-foot kbd { display: inline-flex; min-width: 18px; height: 18px; align-items: center; justify-content: center; border: 1px solid rgb(255 255 255 / 10%); border-radius: 5px; padding: 0 4px; color: rgb(255 255 255 / 55%); background: rgb(255 255 255 / 5%); font: inherit; }
</style>
