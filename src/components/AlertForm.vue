<script setup lang="ts">
import { computed } from 'vue'
import {
  alertMetricOptions,
  describeAlert,
  type AlertDraft,
  type StockConfig,
} from '../settings'

const props = defineProps<{
  draft: AlertDraft
  stocks: StockConfig[]
  /** true = 编辑既有规则（就地展开在那一行下面），false = 新建 */
  editing: boolean
  error: string
}>()

defineEmits<{ submit: []; cancel: [] }>()

/**
 * 草稿的自然语言预览。四个字段散在表单里各填各的，
 * 拼起来到底是条什么规则，读一遍这句最快。
 * describeAlert 与列表里显示的是同一个函数，所见即所得。
 */
const summary = computed(() => {
  if (!props.draft.threshold.trim()) return null
  return describeAlert(
    { ...props.draft, id: '', enabled: true, lastTriggeredDay: null },
    props.stocks,
  )
})
</script>

<template>
  <div
    class="alert-form"
    :class="{ 'is-inline': editing }"
    data-testid="alert-form"
  >
    <div class="alert-form-head">
      <strong>{{ editing ? '编辑提醒' : '新建提醒' }}</strong>
      <!-- 四个字段拼起来到底是条什么规则，一句话摆在最上面 -->
      <span
        v-if="summary"
        class="alert-form-summary"
        data-testid="alert-draft-summary"
      >
        {{ summary }}
      </span>
      <span v-else class="alert-form-summary is-empty">填写阈值后显示规则预览</span>
    </div>

    <div class="alert-field-group">
      <span class="alert-group-label">触发条件</span>
      <div class="alert-condition">
        <label class="alert-field">
          <span>股票</span>
          <select v-model="draft.symbol" data-testid="alert-symbol">
            <option v-for="stock in stocks" :key="stock.symbol" :value="stock.symbol">
              {{ stock.shortName || stock.symbol }} · {{ stock.symbol }}
            </option>
          </select>
        </label>
        <label class="alert-field">
          <span>指标</span>
          <select v-model="draft.metric" data-testid="alert-metric">
            <option
              v-for="option in alertMetricOptions"
              :key="option.value"
              :value="option.value"
            >
              {{ option.label }}
            </option>
          </select>
        </label>
        <div class="alert-field">
          <span>条件</span>
          <div class="segmented-control">
            <button
              type="button"
              :class="{ 'is-active': draft.comparator === 'above' }"
              :aria-pressed="draft.comparator === 'above'"
              data-testid="alert-comparator-above"
              @click="draft.comparator = 'above'"
            >
              ≥ 达到或超过
            </button>
            <button
              type="button"
              :class="{ 'is-active': draft.comparator === 'below' }"
              :aria-pressed="draft.comparator === 'below'"
              data-testid="alert-comparator-below"
              @click="draft.comparator = 'below'"
            >
              ≤ 达到或低于
            </button>
          </div>
        </div>
        <label class="alert-field">
          <span>阈值</span>
          <input
            v-model.trim="draft.threshold"
            data-testid="alert-threshold"
            inputmode="decimal"
            placeholder="例如 3 或 -2000"
          />
          <small class="field-hint">跌幅、亏损用负数</small>
        </label>
      </div>
    </div>

    <div class="alert-field-group">
      <span class="alert-group-label">通知方式</span>
      <div class="alert-field">
        <span>触发频率</span>
        <div class="segmented-control">
          <button
            type="button"
            :class="{ 'is-active': draft.repeat === 'dailyOnce' }"
            :aria-pressed="draft.repeat === 'dailyOnce'"
            data-testid="alert-repeat-daily"
            @click="draft.repeat = 'dailyOnce'"
          >
            每个交易日最多一次
          </button>
          <button
            type="button"
            :class="{ 'is-active': draft.repeat === 'once' }"
            :aria-pressed="draft.repeat === 'once'"
            data-testid="alert-repeat-once"
            @click="draft.repeat = 'once'"
          >
            触发一次后停用
          </button>
        </div>
      </div>
      <label class="runtime-option alert-silent-row">
        <input
          v-model="draft.silent"
          class="toggle-input"
          type="checkbox"
          data-testid="alert-silent"
        />
        <span class="runtime-copy">
          <strong>静默通知</strong>
          <small>只弹横幅不响铃，适合不方便发出声音的场合</small>
        </span>
      </label>
    </div>

    <div class="alert-field-group">
      <span class="alert-group-label">
        通知文案
        <small>选填，填了就完全替换默认的行情文案</small>
      </span>
      <label class="alert-field">
        <span>标题</span>
        <input
          v-model="draft.customTitle"
          data-testid="alert-custom-title"
          maxlength="40"
          placeholder="例如：今天吃了三斤肉"
        />
      </label>
      <label class="alert-field">
        <span>正文</span>
        <input
          v-model="draft.customBody"
          data-testid="alert-custom-body"
          maxlength="80"
          placeholder="留空则使用默认行情文案"
        />
      </label>
    </div>

    <p v-if="error" class="error" role="alert" data-testid="alert-form-error">
      {{ error }}
    </p>
    <div class="alert-form-actions">
      <button
        type="button"
        class="primary"
        data-testid="alert-submit"
        @click="$emit('submit')"
      >
        {{ editing ? '保存修改' : '添加规则' }}
      </button>
      <button type="button" class="ghost" @click="$emit('cancel')">取消</button>
    </div>
    <p class="section-note">
      触发采用「穿越」判定：建规则时若已满足条件不会立刻提醒，
      待数值回落再次越过阈值才触发。休市期间不判定，避免不动的收盘价反复触发。
      规则修改需点击右上角「保存设置」才生效
    </p>
  </div>
</template>
