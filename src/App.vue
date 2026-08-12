<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  errorText,
  firstAlertError,
  firstPositionError,
  pushToast,
  type AlertToast,
  type AppConfig,
  type PortfolioSummary,
  type RefreshStatus,
} from './settings'
import StockSection from './components/StockSection.vue'
import PositionSection from './components/PositionSection.vue'
import AlertSection from './components/AlertSection.vue'
import AlertToasts from './components/AlertToasts.vue'
import DisplaySection from './components/DisplaySection.vue'

const config = ref<AppConfig | null>(null)
const preview = ref('···')
const message = ref('')
const errorMessage = ref('')
const saving = ref(false)
const refreshStatus = ref<RefreshStatus | null>(null)
const portfolio = ref<PortfolioSummary | null>(null)
const showOnboarding = ref(false)
/** 最近一次保存成功的配置快照（JSON），用于判断「有未保存修改」与一键还原。 */
const savedConfig = ref<string | null>(null)

interface AppInfo {
  version: string
  builtAt: string | null
}

const appInfo = ref<AppInfo | null>(null)

let previewTimer: ReturnType<typeof setTimeout> | null = null
let previewRequest = 0
let refreshStatusTimer: ReturnType<typeof setInterval> | null = null
const REFRESH_STATUS_POLL_MS = 5_000

type SettingsSection = 'stock' | 'position' | 'alerts' | 'display' | 'system'

const SECTION_IDS: SettingsSection[] = [
  'stock',
  'position',
  'alerts',
  'display',
  'system',
]
const SECTION_STORAGE_KEY = 'tickerbar.settings.section'

/**
 * 打开设置时停在上次待过的分区，而不是每次都固定跳到某一页。
 * 首次打开落在「股票」——那是这个应用真正的起点，也和首启引导的落点一致。
 */
function restoreSection(): SettingsSection {
  try {
    const stored = localStorage.getItem(SECTION_STORAGE_KEY)
    if (stored && SECTION_IDS.includes(stored as SettingsSection)) {
      return stored as SettingsSection
    }
  } catch {
    // localStorage 被禁用时按默认处理即可
  }
  return 'stock'
}

const activeSection = ref<SettingsSection>(restoreSection())

watch(activeSection, (section) => {
  try {
    localStorage.setItem(SECTION_STORAGE_KEY, section)
  } catch {
    // 记不住就下次仍从「股票」开始，不影响使用
  }
})

const settingsSections = [
  {
    id: 'stock',
    title: '股票与行情',
    subtitle: '管理关注的股票，列表顺序即菜单栏下拉顺序',
  },
  {
    id: 'position',
    title: '持仓计算',
    subtitle: '用本机持仓数据计算实时收益与合计',
  },
  {
    id: 'alerts',
    title: '提醒',
    subtitle: '价格、涨跌幅或收益到达阈值时发系统通知，文案可完全自定义',
  },
  {
    id: 'display',
    title: '菜单栏展示',
    subtitle: '决定菜单栏显示哪些数据、以什么格式显示',
  },
  {
    id: 'system',
    title: '系统设置',
    subtitle: '控制 TickerBar 在 macOS 中的启动方式',
  },
] as const

const positionError = computed(() =>
  firstPositionError(config.value?.stocks ?? []),
)

const alertError = computed(() =>
  firstAlertError(config.value?.alerts ?? [], config.value?.stocks ?? []),
)

const activeSectionMeta = computed(
  () =>
    settingsSections.find((section) => section.id === activeSection.value) ??
    settingsSections[0],
)

/**
 * 输入层错误（持仓半填、格式非法）会让配置无法被后端反序列化，
 * 此时一律不发 IPC——否则用户看到的是 serde 抛出的英文类型错误。
 */
const draftError = computed(() => positionError.value)

const isDirty = computed(
  () =>
    savedConfig.value !== null &&
    JSON.stringify(config.value) !== savedConfig.value,
)

function snapshot(value: AppConfig | null): string | null {
  return value === null ? null : JSON.stringify(value)
}

/** 放弃尚未保存的修改，回到最近一次保存的状态。 */
function discardChanges() {
  if (!savedConfig.value) return
  config.value = JSON.parse(savedConfig.value) as AppConfig
  message.value = '已还原到上次保存的设置'
  errorMessage.value = ''
  void refreshPreview()
}

function handleStockSelected() {
  message.value = ''
  void refreshPreview()
}

async function loadConfig() {
  try {
    config.value = await invoke<AppConfig>('get_config')
    savedConfig.value = snapshot(config.value)
    await refreshPreview()
  } catch (error) {
    errorMessage.value = errorText(error)
  }
}

async function loadFirstRun() {
  try {
    showOnboarding.value = await invoke<boolean>('get_first_run')
  } catch {
    showOnboarding.value = false
  }
}

// 版本与构建时间：覆盖安装后用它确认跑的是不是新包。
async function loadAppInfo() {
  try {
    appInfo.value = await invoke<AppInfo>('get_app_info')
  } catch {
    appInfo.value = null
  }
}

const onboardingStart = ref<HTMLButtonElement | null>(null)

// 引导层是模态：出现时把焦点移进来，配合背景 inert 阻断 Tab 穿透。
watch(showOnboarding, (visible) => {
  if (visible) {
    void nextTick(() => onboardingStart.value?.focus())
  }
})

function completeOnboarding() {
  showOnboarding.value = false
  activeSection.value = 'stock'
  // 翻转失败只影响同一会话内重开设置窗是否再弹引导，忽略即可；
  // 下次启动时配置文件已存在，自然不会再触发。
  void invoke('dismiss_first_run').catch(() => {})
}

// 刷新状态是旁路信息：加载失败静默跳过，不打扰主流程。
async function loadRefreshStatus() {
  try {
    refreshStatus.value = await invoke<RefreshStatus>('get_refresh_status')
  } catch {
    refreshStatus.value = null
  }
}

// 持仓汇总是旁路信息：输入到一半非法时保留上一份结果，不打断编辑。
async function refreshPortfolio() {
  if (!config.value || draftError.value) return
  try {
    portfolio.value = await invoke<PortfolioSummary>('preview_portfolio', {
      config: config.value,
    })
  } catch {
    // 保留上一份汇总
  }
}

async function refreshPreview() {
  if (!config.value) return

  void refreshPortfolio()
  // 输入还没填完整就别打后端了：空的数量/成本无法反序列化成 Decimal，
  // 硬发过去只会换回一条 serde 的英文类型错误，对用户毫无帮助。
  // 提示交给出错那一行就地显示，这里不再往顶栏堆红字——边填边报太吵。
  if (draftError.value) {
    preview.value = '待补全设置'
    previewRequest += 1
    return
  }

  // 请求序号防止乱序响应把预览覆盖成旧值。
  const request = ++previewRequest
  try {
    const title = await invoke<string>('preview_title', {
      config: config.value,
    })
    if (request !== previewRequest) return
    preview.value = title
    errorMessage.value = ''
  } catch (error) {
    if (request !== previewRequest) return
    preview.value = '预览不可用'
    errorMessage.value = errorText(error)
  }
}

// 文本输入走防抖版本，避免每个键击都打一次后端。
function schedulePreview() {
  if (previewTimer) clearTimeout(previewTimer)
  previewTimer = setTimeout(() => {
    previewTimer = null
    void refreshPreview()
  }, 250)
}

function startRefreshStatusPolling() {
  if (refreshStatusTimer) return
  refreshStatusTimer = setInterval(() => {
    void loadRefreshStatus()
  }, REFRESH_STATUS_POLL_MS)
}

function stopRefreshStatusPolling() {
  if (refreshStatusTimer) clearInterval(refreshStatusTimer)
  refreshStatusTimer = null
}

// 设置窗最小化/隐藏时暂停轮询，回到前台立即拉一次再恢复，不做无人看的 IPC。
function handleVisibilityChange() {
  if (document.hidden) {
    stopRefreshStatusPolling()
  } else {
    void loadRefreshStatus()
    startRefreshStatusPolling()
  }
}

/**
 * 提醒触发时后端广播事件，这里补一条窗口内 Toast。
 * 设置窗口在最前台时 macOS 会把系统横幅吞掉只留通知中心，
 * 而用户恰恰是开着这个窗口在等提醒。
 */
const toasts = ref<AlertToast[]>([])
let toastSeq = 0
let unlistenAlert: UnlistenFn | null = null

function dismissToast(key: number) {
  toasts.value = toasts.value.filter((toast) => toast.key !== key)
}

function startAlertToasts() {
  void listen<{ title: string; body: string }>('alert-triggered', (event) => {
    toastSeq += 1
    toasts.value = pushToast(toasts.value, {
      key: toastSeq,
      title: event.payload.title,
      body: event.payload.body,
    })
  })
    .then((unlisten) => {
      // 监听注册是异步的，卸载可能先发生——那就地取消，别留悬挂监听
      if (unmounted) unlisten()
      else unlistenAlert = unlisten
    })
    .catch(() => {})
}

let unmounted = false

onBeforeUnmount(() => {
  unmounted = true
  if (previewTimer) clearTimeout(previewTimer)
  previewTimer = null
  previewRequest += 1
  stopRefreshStatusPolling()
  unlistenAlert?.()
  unlistenAlert = null
  document.removeEventListener('visibilitychange', handleVisibilityChange)
})

async function save() {
  if (!config.value || saving.value) return
  if (positionError.value) {
    errorMessage.value = positionError.value
    return
  }
  // 已保存的提醒规则也要全量复核：关掉持仓后依赖持仓的规则会变成死规则。
  if (alertError.value) {
    errorMessage.value = alertError.value
    return
  }
  saving.value = true
  message.value = ''
  errorMessage.value = ''

  try {
    config.value = await invoke<AppConfig>('save_user_config', {
      config: config.value,
    })
    savedConfig.value = snapshot(config.value)
    message.value = '设置已保存'
    await refreshPreview()
  } catch (error) {
    errorMessage.value = errorText(error)
  } finally {
    saving.value = false
  }
}

onMounted(() => {
  void loadConfig()
  void loadFirstRun()
  void loadAppInfo()
  void loadRefreshStatus()
  startRefreshStatusPolling()
  startAlertToasts()
  document.addEventListener('visibilitychange', handleVisibilityChange)
})
</script>

<template>
  <main class="settings-shell">
    <AlertToasts :toasts="toasts" @dismiss="dismissToast" />
    <div v-if="showOnboarding" class="onboarding-backdrop" data-testid="onboarding">
      <div
        class="onboarding-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-title"
      >
        <h2 id="onboarding-title">欢迎使用 TickerBar</h2>
        <p class="onboarding-lead">
          一个常驻菜单栏的本地行情小工具。花 30 秒完成初始设置：
        </p>
        <ol class="onboarding-steps">
          <li>
            <strong>添加你的股票</strong>
            <span>在「股票」页搜索名称或代码。当前放了小米作为示例，可随时移除</span>
          </li>
          <li>
            <strong>配置持仓（可选）</strong>
            <span>在「持仓」页填写数量与成本，菜单栏即可显示实时收益</span>
          </li>
          <li>
            <strong>设置提醒（可选）</strong>
            <span>价格、涨跌幅或收益到达阈值时发系统通知，通知文案可完全自定义</span>
          </li>
        </ol>
        <p class="onboarding-note">
          行情来自腾讯免费接口，可能存在延迟，仅供参考，不构成投资建议。
          所有配置与持仓数据只保存在本机，不读取任何券商账户
        </p>
        <button
          ref="onboardingStart"
          type="button"
          class="primary onboarding-start"
          data-testid="onboarding-start"
          @click="completeOnboarding"
        >
          开始设置
        </button>
      </div>
    </div>

    <!-- inert 是布尔属性，必须在 falsy 时移除（undefined），绑 false 会渲染 inert="false" 仍然生效 -->
    <div v-if="!config" class="loading" :inert="showOnboarding || undefined">
      <span v-if="errorMessage" class="error">{{ errorMessage }}</span>
      <span v-else>正在读取设置…</span>
    </div>

    <!-- 引导层展示期间背景表单 inert：视觉遮罩挡不住 Tab，inert 才能真正阻断键盘焦点。 -->
    <form
      v-else
      class="settings-layout"
      :inert="showOnboarding || undefined"
      @submit.prevent="save"
    >
      <aside class="settings-sidebar">
        <div class="sidebar-brand">
          <div class="brand-mark" aria-hidden="true">
            <svg viewBox="0 0 32 32" role="presentation">
              <path d="M6 21.5 12 15l4 3.5L25.5 8" />
              <path d="M21 8h4.5v4.5" />
            </svg>
          </div>
          <h1>TickerBar</h1>
        </div>

        <nav class="sidebar-nav" aria-label="设置分类">
          <button
            data-testid="nav-stock"
            type="button"
            :class="{ 'is-active': activeSection === 'stock' }"
            @click="activeSection = 'stock'"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M5 19V10M12 19V5M19 19v-7" />
            </svg>
            <span>股票</span>
          </button>
          <button
            data-testid="nav-position"
            type="button"
            :class="{ 'is-active': activeSection === 'position' }"
            @click="activeSection = 'position'"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <rect x="4" y="7" width="16" height="12" rx="2" />
              <path d="M9 7V5h6v2M4 12h16" />
            </svg>
            <span>持仓</span>
          </button>
          <button
            data-testid="nav-alerts"
            type="button"
            :class="{ 'is-active': activeSection === 'alerts' }"
            @click="activeSection = 'alerts'"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <path d="M18 16v-5a6 6 0 1 0-12 0v5l-1.5 2h15z" />
              <path d="M10.5 20a1.5 1.5 0 0 0 3 0" />
            </svg>
            <span>提醒</span>
          </button>
          <button
            data-testid="nav-display"
            type="button"
            :class="{ 'is-active': activeSection === 'display' }"
            @click="activeSection = 'display'"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <rect x="3" y="4" width="18" height="13" rx="2" />
              <path d="M8 21h8M12 17v4" />
            </svg>
            <span>显示</span>
          </button>
          <button
            data-testid="nav-system"
            type="button"
            :class="{ 'is-active': activeSection === 'system' }"
            @click="activeSection = 'system'"
          >
            <svg viewBox="0 0 24 24" aria-hidden="true">
              <circle cx="12" cy="12" r="3" />
              <path d="M19 13.5v-3l-2-.7-.7-1.7.9-1.9-2.1-2.1-1.9.9-1.7-.7-.7-2h-3l-.7 2-1.7.7-1.9-.9-2.1 2.1.9 1.9-.7 1.7-2 .7v3l2 .7.7 1.7-.9 1.9 2.1 2.1 1.9-.9 1.7.7.7 2h3l.7-2 1.7-.7 1.9.9 2.1-2.1-.9-1.9.7-1.7z" />
            </svg>
            <span>系统</span>
          </button>
        </nav>

        <div class="sidebar-footer">
          <span>本地运行 · 不上传</span>
          <strong data-testid="app-build">
            v{{ appInfo?.version ?? '—' }}
            <template v-if="appInfo?.builtAt">
              · 构建于 {{ appInfo.builtAt }}
            </template>
          </strong>
        </div>
      </aside>

      <section class="settings-workspace">
        <header class="workspace-header">
          <div class="workspace-title">
            <h2 data-testid="workspace-title">{{ activeSectionMeta.title }}</h2>
            <p>{{ activeSectionMeta.subtitle }}</p>
          </div>
          <div class="workspace-actions">
            <div class="feedback" aria-live="polite">
              <span v-if="message" class="success">{{ message }}</span>
              <span v-else-if="errorMessage" class="error">
                {{ errorMessage }}
              </span>
              <span v-else-if="isDirty" class="pending" data-testid="dirty-hint">
                有未保存的修改
              </span>
            </div>
            <button
              type="button"
              class="ghost"
              data-testid="discard"
              :disabled="!isDirty || saving"
              @click="discardChanges"
            >
              放弃修改
            </button>
            <button data-testid="save" class="primary" :disabled="saving">
              {{ saving ? '保存中…' : '保存设置' }}
            </button>
          </div>
        </header>

        <div class="workspace-scroll">
          <div class="preview-card">
            <div class="preview-meta">
              <span class="preview-label">
                <i aria-hidden="true"></i>
                实时预览
              </span>
              <small :class="{ warning: preview.length > 18 }">
                {{ preview.length }} 个字符
                <template v-if="preview.length > 18">
                  · 刘海屏可能过长
                </template>
              </small>
            </div>
            <strong data-testid="tray-preview">{{ preview }}</strong>
            <div class="preview-status" data-testid="refresh-status">
              <span v-if="refreshStatus?.lastSuccessAt">
                最后更新 {{ refreshStatus.lastSuccessAt }}
              </span>
              <span v-else>尚未成功刷新</span>
              <span
                v-if="refreshStatus?.lastError"
                class="preview-status-error"
                data-testid="refresh-status-error"
              >
                上次错误 {{ refreshStatus.lastError }}
              </span>
            </div>
          </div>

          <StockSection
            v-show="activeSection === 'stock'"
            :config="config"
            @stock-selected="handleStockSelected"
            @schedule-preview="schedulePreview"
          />

          <PositionSection
            v-show="activeSection === 'position'"
            :config="config"
            :summary="portfolio"
            @schedule-preview="schedulePreview"
          />

          <AlertSection v-show="activeSection === 'alerts'" :config="config" />

          <DisplaySection
            v-show="activeSection === 'display'"
            :config="config"
            @refresh-preview="refreshPreview"
          />

          <section
            v-show="activeSection === 'system'"
            class="settings-page"
            data-testid="page-system"
          >
            <div data-testid="runtime-settings" class="runtime-settings">
              <div class="section-label">
                <h3>运行选项</h3>
                <p>这些设置控制应用在 macOS 中的行为</p>
              </div>
              <div class="runtime-options">
                <label class="runtime-option">
                  <input
                    v-model="config.launchAtLogin"
                    class="toggle-input"
                    type="checkbox"
                  />
                  <span class="runtime-copy">
                    <strong>登录时启动</strong>
                    <small>进入 macOS 后自动恢复菜单栏行情</small>
                  </span>
                </label>
              </div>
            </div>
          </section>
        </div>
      </section>
    </form>
  </main>
</template>
