<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import {
  applyPreset,
  errorText,
  hasAnyNumericOption,
  metricCapability,
  metricOptions,
  moveMetric,
  toggleMetric,
  type AppConfig,
  type DisplayMetric,
  type DisplayPreset,
  type MetricConfig,
} from '../settings'
import { usePointerDrag } from '../hooks/usePointerDrag'

const props = defineProps<{ config: AppConfig }>()
const emit = defineEmits<{
  'refresh-preview': []
}>()

const expandedMetrics = ref(new Set<DisplayMetric>())
const displayError = ref('')

const {
  draggingIndex,
  dropTargetIndex,
  begin: beginPointerDrag,
  move: movePointerDrag,
  finish: finishPointerDrag,
  cancel: cancelPointerDrag,
  dispose: disposePointerDrag,
} = usePointerDrag((sourceIndex, targetIndex) => {
  props.config.display.items = moveMetric(
    props.config.display.items,
    sourceIndex,
    targetIndex,
  )
  emit('refresh-preview')
})

const metricGroups = [
  { id: 'quote', label: '行情', hint: '价格与日内' },
  { id: 'position', label: '持仓', hint: '本地收益计算' },
  { id: 'status', label: '状态', hint: '连接与时间' },
] as const

const precisionOptions = [0, 1, 2, 3] as const
const formatOptions = [
  { value: 'plain', label: '普通' },
  { value: 'sign', label: '正负号' },
  { value: 'arrow', label: '箭头' },
] as const
const compactOptions = [
  { value: 'none', label: '关闭' },
  { value: 'chinese', label: '万/亿' },
  { value: 'western', label: 'K/M' },
] as const

const selectedMetrics = computed(
  () => new Set(props.config.display.items.map((item) => item.metric)),
)

function metricLabel(metric: DisplayMetric): string {
  return metricOptions.find((option) => option.metric === metric)?.label ?? metric
}

function metricFormatLabel(item: MetricConfig): string {
  if (item.directionArrow) return '箭头'
  return item.showSign ? '正负号' : '普通'
}

function metricCompactLabel(item: MetricConfig): string {
  if (item.compactStyle === 'chinese') return '万/亿'
  if (item.compactStyle === 'western') return 'K/M'
  return '不缩写'
}

/** 摘要行只列出该指标真正适用的调整项，纯文本项直接说明无需调整。 */
function metricSummary(item: MetricConfig): string {
  const capability = metricCapability(item.metric)
  const parts: string[] = []
  if (capability.precision) parts.push(`${item.precision} 位`)
  if (capability.format) parts.push(metricFormatLabel(item))
  if (capability.compact) parts.push(metricCompactLabel(item))
  if (item.label) parts.push(`标签「${item.label}」`)
  return parts.length ? parts.join(' · ') : '文本内容，无需格式调整'
}

function metricDetailsExpanded(metric: DisplayMetric): boolean {
  return expandedMetrics.value.has(metric)
}

function toggleMetricDetails(metric: DisplayMetric) {
  const next = new Set(expandedMetrics.value)
  if (next.has(metric)) next.delete(metric)
  else next.add(metric)
  expandedMetrics.value = next
}

function setItemPrecision(item: MetricConfig, precision: number) {
  item.precision = precision
  updateItem()
}

function setItemFormat(item: MetricConfig, format: 'plain' | 'sign' | 'arrow') {
  item.directionArrow = format === 'arrow'
  item.showSign = format === 'sign'
  updateItem()
}

function setItemCompactStyle(
  item: MetricConfig,
  compactStyle: MetricConfig['compactStyle'],
) {
  item.compactStyle = compactStyle
  updateItem()
}

function setMetric(metric: DisplayMetric, event: Event) {
  const selected = (event.target as HTMLInputElement).checked

  try {
    props.config.display.items = toggleMetric(
      props.config.display.items,
      metric,
      selected,
    )
    displayError.value = ''
    emit('refresh-preview')
  } catch (error) {
    displayError.value = errorText(error)
  }
}

function selectPreset(preset: DisplayPreset) {
  props.config.display.items = applyPreset(preset)
  emit('refresh-preview')
}

function move(index: number, delta: number) {
  props.config.display.items = moveMetric(
    props.config.display.items,
    index,
    index + delta,
  )
  emit('refresh-preview')
}

function updateItem() {
  emit('refresh-preview')
}

function updateLabel(item: MetricConfig, event: Event) {
  const value = (event.target as HTMLInputElement).value.trim()
  item.label = value || null
  updateItem()
}

onBeforeUnmount(disposePointerDrag)
</script>

<template>
  <section class="settings-page display-page" data-testid="page-display">
    <div class="preset-row">
      <span>快速预设</span>
      <button type="button" @click="selectPreset('price')">
        仅价格
      </button>
      <button type="button" @click="selectPreset('priceChange')">
        价格 + 涨跌
      </button>
      <button
        type="button"
        data-testid="preset-position"
        @click="selectPreset('position')"
      >
        持仓收益
      </button>
    </div>

    <div class="display-workbench" data-testid="display-workbench">
      <div
        class="selected-list workbench-scroll"
        data-testid="display-order-scroll"
        role="region"
        aria-label="菜单栏显示顺序"
        tabindex="0"
      >
        <div class="list-heading">
          <div>
            <h3>显示顺序</h3>
            <p>拖动或使用箭头调整菜单栏排列</p>
          </div>
          <label>
            <span>分隔符</span>
            <select
              v-model="config.display.separator"
              @change="emit('refresh-preview')"
            >
              <option value=" ">空格</option>
              <option value=" · ">中点 ·</option>
              <option value=" | ">竖线 |</option>
              <option value=" / ">斜线 /</option>
            </select>
          </label>
        </div>

        <div class="selected-stack">
          <article
            v-for="(item, index) in config.display.items"
            :key="item.metric"
            :data-testid="`selected-${item.metric}`"
            :data-drag-index="index"
            class="selected-item"
            :class="{
              'is-dragging': draggingIndex === index,
              'is-drop-target': dropTargetIndex === index,
            }"
          >
            <div class="item-summary">
              <span
                class="drag-handle"
                aria-hidden="true"
                :data-testid="`drag-handle-${item.metric}`"
                @pointerdown.stop="beginPointerDrag(index, $event)"
                @pointermove="movePointerDrag"
                @pointerup="finishPointerDrag"
                @pointercancel="cancelPointerDrag"
              >
                ⠿
              </span>
              <div class="item-identity">
                <strong>{{ metricLabel(item.metric) }}</strong>
                <span>{{ metricSummary(item) }}</span>
              </div>
              <button
                type="button"
                class="item-disclosure"
                :class="{
                  'is-expanded': metricDetailsExpanded(item.metric),
                }"
                :data-testid="`item-toggle-${item.metric}`"
                :aria-expanded="metricDetailsExpanded(item.metric)"
                @click="toggleMetricDetails(item.metric)"
              >
                <span>调整</span>
                <svg viewBox="0 0 16 16" aria-hidden="true">
                  <path d="m4 6 4 4 4-4" />
                </svg>
              </button>
              <div class="move-buttons">
                <button
                  type="button"
                  aria-label="上移"
                  :disabled="index === 0"
                  @click="move(index, -1)"
                >
                  ↑
                </button>
                <button
                  type="button"
                  aria-label="下移"
                  :disabled="index === config.display.items.length - 1"
                  @click="move(index, 1)"
                >
                  ↓
                </button>
              </div>
            </div>

            <div
              v-show="metricDetailsExpanded(item.metric)"
              :data-testid="`item-options-${item.metric}`"
              class="item-options"
            >
              <div
                v-if="metricCapability(item.metric).precision"
                class="option-field"
              >
                <span>小数位数</span>
                <div class="segmented-control">
                  <button
                    v-for="precision in precisionOptions"
                    :key="precision"
                    type="button"
                    :class="{ 'is-active': item.precision === precision }"
                    :aria-pressed="item.precision === precision"
                    :data-testid="`precision-${item.metric}-${precision}`"
                    @click="setItemPrecision(item, precision)"
                  >
                    {{ precision }}
                  </button>
                </div>
              </div>
              <div
                v-if="metricCapability(item.metric).format"
                class="option-field"
              >
                <span>数值格式</span>
                <div class="segmented-control">
                  <button
                    v-for="format in formatOptions"
                    :key="format.value"
                    type="button"
                    :class="{
                      'is-active': metricFormatLabel(item) === format.label,
                    }"
                    :aria-pressed="
                      metricFormatLabel(item) === format.label
                    "
                    :data-testid="`format-${item.metric}-${format.value}`"
                    @click="setItemFormat(item, format.value)"
                  >
                    {{ format.label }}
                  </button>
                </div>
              </div>
              <div
                v-if="metricCapability(item.metric).compact"
                class="option-field"
              >
                <span>数值缩写</span>
                <div class="segmented-control">
                  <button
                    v-for="compact in compactOptions"
                    :key="compact.value"
                    type="button"
                    :class="{
                      'is-active': item.compactStyle === compact.value,
                    }"
                    :aria-pressed="
                      item.compactStyle === compact.value
                    "
                    :data-testid="`compact-${item.metric}-${compact.value}`"
                    @click="setItemCompactStyle(item, compact.value)"
                  >
                    {{ compact.label }}
                  </button>
                </div>
              </div>
              <label class="option-field short-label-field">
                <span>短标签</span>
                <input
                  :value="item.label ?? ''"
                  :data-testid="`label-${item.metric}`"
                  maxlength="3"
                  placeholder="无"
                  @input="updateLabel(item, $event)"
                />
              </label>
              <p
                v-if="!hasAnyNumericOption(item.metric)"
                class="option-hint"
                :data-testid="`item-text-only-${item.metric}`"
              >
                该项是文本内容，只能加短标签，没有小数位与数值格式可调
              </p>
            </div>
          </article>
        </div>

        <div class="status-options">
          <label>
            <input
              v-model="config.display.appendClosedStatus"
              type="checkbox"
              @change="emit('refresh-preview')"
            />
            收盘时附加“·收”
          </label>
          <label>
            <input
              v-model="config.display.appendDelayedStatus"
              type="checkbox"
              @change="emit('refresh-preview')"
            />
            延迟行情附加“·延”
          </label>
        </div>
      </div>

      <aside
        class="metric-picker workbench-scroll"
        data-testid="available-data-scroll"
        role="region"
        aria-label="可添加的数据"
        tabindex="0"
      >
        <div class="metric-picker-heading">
          <h3>添加数据</h3>
          <span data-testid="metric-selection-count">
            已选 {{ selectedMetrics.size }} 项
          </span>
        </div>
        <p v-if="displayError" class="error" role="alert">
          {{ displayError }}
        </p>
        <section
          v-for="group in metricGroups"
          :key="group.id"
          :data-testid="`metric-group-${group.id}`"
          class="metric-group"
        >
          <header class="metric-group-heading">
            <strong>{{ group.label }}</strong>
            <small>{{ group.hint }}</small>
          </header>
          <div class="metric-grid">
            <label
              v-for="option in metricOptions.filter(
                (item) => item.group === group.label,
              )"
              :key="option.metric"
              :data-testid="`metric-card-${option.metric}`"
              class="metric-checkbox"
              :class="{
                'is-selected': selectedMetrics.has(option.metric),
              }"
            >
              <input
                type="checkbox"
                :data-testid="`metric-${option.metric}`"
                :checked="selectedMetrics.has(option.metric)"
                @change="setMetric(option.metric, $event)"
              />
              <span>{{ option.label }}</span>
            </label>
          </div>
        </section>
      </aside>
    </div>
  </section>
</template>
