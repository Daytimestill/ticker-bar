<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import {
  errorText,
  MAX_ALERTS,
  alertDraftError,
  describeAlert,
  type AlertDraft,
  type AlertRule,
  type AppConfig,
} from '../settings'
import AlertForm from './AlertForm.vue'

const props = defineProps<{ config: AppConfig }>()

function emptyDraft(): AlertDraft {
  return {
    symbol: props.config.stocks[props.config.activeStock]?.symbol ?? '',
    metric: 'changePercent',
    comparator: 'above',
    threshold: '',
    repeat: 'dailyOnce',
    silent: false,
    customTitle: '',
    customBody: '',
  }
}

const draft = ref<AlertDraft>(emptyDraft())
// null = 新建；否则为正在编辑的规则 id
const editingId = ref<string | null>(null)
const formError = ref('')

const formVisible = ref(false)

const draftValidationError = computed(() =>
  alertDraftError(draft.value, props.config.stocks),
)

function startCreate() {
  draft.value = emptyDraft()
  editingId.value = null
  formError.value = ''
  formVisible.value = true
}

function startEdit(rule: AlertRule) {
  draft.value = {
    symbol: rule.symbol,
    metric: rule.metric,
    comparator: rule.comparator,
    threshold: rule.threshold,
    repeat: rule.repeat,
    silent: rule.silent,
    customTitle: rule.customTitle ?? '',
    customBody: rule.customBody ?? '',
  }
  editingId.value = rule.id
  formError.value = ''
  formVisible.value = true
}

function cancelForm() {
  formVisible.value = false
  editingId.value = null
  formError.value = ''
}

function submitForm() {
  const error = draftValidationError.value
  if (error) {
    formError.value = error
    return
  }
  if (!editingId.value && props.config.alerts.length >= MAX_ALERTS) {
    formError.value = `最多支持 ${MAX_ALERTS} 条提醒规则`
    return
  }
  const payload = {
    symbol: draft.value.symbol,
    metric: draft.value.metric,
    comparator: draft.value.comparator,
    threshold: draft.value.threshold.trim(),
    repeat: draft.value.repeat,
    silent: draft.value.silent,
    customTitle: draft.value.customTitle.trim() || null,
    customBody: draft.value.customBody.trim() || null,
  }
  if (editingId.value) {
    props.config.alerts = props.config.alerts.map((rule) =>
      rule.id === editingId.value
        ? // 编辑视为新规则：清空触发记录，重新按穿越语义武装。
          { ...rule, ...payload, enabled: true, lastTriggeredDay: null }
        : rule,
    )
  } else {
    props.config.alerts = [
      ...props.config.alerts,
      {
        id: crypto.randomUUID(),
        ...payload,
        enabled: true,
        lastTriggeredDay: null,
      },
    ]
  }
  cancelForm()
}

function removeRule(id: string) {
  props.config.alerts = props.config.alerts.filter((rule) => rule.id !== id)
}

function toggleRule(rule: AlertRule, event: Event) {
  const enabled = (event.target as HTMLInputElement).checked
  props.config.alerts = props.config.alerts.map((candidate) =>
    candidate.id === rule.id
      ? { ...candidate, enabled, lastTriggeredDay: null }
      : candidate,
  )
}

// 留给用户切走窗口的时间，见 sendTest 的注释
const TEST_DELAY_SECONDS = 3

const testingId = ref<string | null>(null)
const countdown = ref(0)
const testResult = ref('')
const testError = ref('')
let countdownTimer: ReturnType<typeof setInterval> | null = null

function stopCountdown() {
  if (countdownTimer === null) return
  clearInterval(countdownTimer)
  countdownTimer = null
}

// 组件卸载时倒计时还在跑的话，定时器会打到已销毁的组件上
onBeforeUnmount(stopCountdown)

/**
 * 试发：跳过穿越判定直接发一条通知，文案与真实触发完全一致。
 * 休市时提醒不会自己触发，只能靠它验证通知权限与伪装文案。
 *
 * 倒计时不是装饰。macOS 对「发通知时自己正在前台」的 App 不弹横幅，
 * 只把通知折叠进通知中心，而试发必然是在设置窗口里点的——
 * 不留切走的时间，这个按钮就永远测不出它要测的那件事。
 */
function sendTest(rule: AlertRule) {
  if (testingId.value) return
  testingId.value = rule.id
  testResult.value = ''
  testError.value = ''
  countdown.value = TEST_DELAY_SECONDS
  countdownTimer = setInterval(() => {
    countdown.value -= 1
    if (countdown.value > 0) return
    stopCountdown()
    void fireTest(rule)
  }, 1000)
}

async function fireTest(rule: AlertRule) {
  try {
    await invoke('send_test_alert', { rule })
    // 内容长什么样，右上角的 Toast 已经当场展示了，这里不必复述
    testResult.value = '已发送'
  } catch (error) {
    testError.value = errorText(error)
  } finally {
    testingId.value = null
  }
}

function ruleTags(rule: AlertRule): string[] {
  const tags = [rule.repeat === 'dailyOnce' ? '每日一次' : '一次性']
  if (rule.silent) tags.push('静默')
  if (rule.customTitle || rule.customBody) tags.push('伪装文案')
  return tags
}
</script>

<template>
  <section class="settings-page" data-testid="page-alerts">
    <div class="page-toolbar">
      <span class="page-kicker">提醒规则</span>
      <button
        type="button"
        class="alert-create"
        data-testid="alert-create"
        @click="startCreate"
      >
        新建提醒
      </button>
    </div>

    <div v-if="config.alerts.length" class="alert-list" data-testid="alert-list">
      <template v-for="rule in config.alerts" :key="rule.id">
      <div
        class="alert-row"
        :class="{ 'is-disabled': !rule.enabled, 'is-editing': editingId === rule.id }"
        :data-testid="`alert-row-${rule.id}`"
      >
        <label class="alert-toggle">
          <input
            type="checkbox"
            class="toggle-input"
            :checked="rule.enabled"
            :data-testid="`alert-enabled-${rule.id}`"
            @change="toggleRule(rule, $event)"
          />
        </label>
        <div class="alert-copy">
          <strong>{{ describeAlert(rule, config.stocks) }}</strong>
          <small>
            <span v-for="tag in ruleTags(rule)" :key="tag" class="alert-tag">
              {{ tag }}
            </span>
          </small>
        </div>
        <div class="alert-actions">
          <!-- 文案不随状态变化：本地通知瞬间返回，切成「发送中…」只会让按钮宽度抖一下 -->
          <button
            type="button"
            :data-testid="`alert-test-${rule.id}`"
            :disabled="testingId !== null"
            @click="sendTest(rule)"
          >
            试发
          </button>
          <button
            type="button"
            :data-testid="`alert-edit-${rule.id}`"
            @click="startEdit(rule)"
          >
            编辑
          </button>
          <button
            type="button"
            :data-testid="`alert-remove-${rule.id}`"
            @click="removeRule(rule.id)"
          >
            删除
          </button>
        </div>
      </div>

      <!--
        编辑表单就地展开在被编辑那一行的正下方。
        原来它固定渲染在页面最末尾，改第一条规则时表单出现在整个列表之后，
        中间隔着一大截，看不出在改哪一条。
      -->
      <AlertForm
        v-if="editingId === rule.id"
        :draft="draft"
        :stocks="config.stocks"
        :editing="true"
        :error="formError"
        @submit="submitForm"
        @cancel="cancelForm"
      />
      </template>
    </div>
    <p v-if="!config.alerts.length" class="section-note">
      还没有提醒规则。新建一条，例如「今日涨跌幅 ≥ 3%」或「持仓收益 ≤ -2000」，
      触发时会弹出 macOS 系统通知
    </p>

    <!-- 新建表单没有归属的行，紧接在列表后面——排到试发反馈区之后会被顶开一大截 -->
    <AlertForm
      v-if="formVisible && !editingId"
      :draft="draft"
      :stocks="config.stocks"
      :editing="false"
      :error="formError"
      @submit="submitForm"
      @cancel="cancelForm"
    />

    <!-- 常驻占位：提示出现/消失时不再把下方内容顶来顶去 -->
    <div v-if="config.alerts.length" class="alert-test-feedback" aria-live="polite">
      <span
        v-if="countdown > 0"
        class="countdown"
        data-testid="alert-test-countdown"
      >
        {{ countdown }} 秒后发送，请先切到其他窗口
      </span>
      <span v-else-if="testError" class="error" data-testid="alert-test-error">
        {{ testError }}
      </span>
      <span v-else-if="testResult" class="success" data-testid="alert-test-result">
        {{ testResult }}
      </span>
    </div>

    <!-- 常驻在页面最后：横幅去哪了是这一页最容易让人以为「提醒坏了」的地方 -->
    <p class="section-note" data-testid="alert-delivery-note">
      <strong>通知去向</strong>：窗口关闭时弹桌面横幅；窗口停在最前台时
      macOS 不弹横幅，通知进通知中心，改由右上角卡片提示。
      试发倒数 {{ TEST_DELAY_SECONDS }} 秒，便于切走窗口验证横幅
    </p>
  </section>
</template>
