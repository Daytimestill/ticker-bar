import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { errorText, type StockSearchResult } from '../settings'

const SEARCH_DEBOUNCE_MS = 250
const CLOSE_DELAY_MS = 120

/**
 * 股票搜索组合式函数：防抖搜索、乱序响应保护、键盘导航。
 * 选中后清空搜索框并把结果交给 onSelected，配置由调用方决定怎么改。
 */
export function useStockSearch(onSelected: (result: StockSearchResult) => void) {
  const query = ref('')
  const results = ref<StockSearchResult[]>([])
  const loading = ref(false)
  const open = ref(false)
  const error = ref('')
  const activeIndex = ref(-1)

  let searchTimer: ReturnType<typeof setTimeout> | null = null
  let closeTimer: ReturnType<typeof setTimeout> | null = null
  let request = 0

  async function perform(term: string) {
    const current = ++request
    loading.value = true
    error.value = ''

    try {
      const found = await invoke<StockSearchResult[]>('search_stocks', {
        query: term,
      })
      if (current !== request) return
      results.value = found
      activeIndex.value = found.length > 0 ? 0 : -1
      open.value = true
    } catch (cause) {
      if (current !== request) return
      results.value = []
      activeIndex.value = -1
      open.value = true
      error.value = errorText(cause)
    } finally {
      if (current === request) loading.value = false
    }
  }

  function schedule() {
    if (searchTimer) clearTimeout(searchTimer)
    request += 1
    const term = query.value.trim()
    if (!term) {
      loading.value = false
      results.value = []
      open.value = false
      error.value = ''
      return
    }
    open.value = true
    loading.value = true
    results.value = []
    activeIndex.value = -1
    searchTimer = setTimeout(() => {
      searchTimer = null
      void perform(term)
    }, SEARCH_DEBOUNCE_MS)
  }

  function select(result: StockSearchResult) {
    if (searchTimer) clearTimeout(searchTimer)
    searchTimer = null
    request += 1
    loading.value = false
    results.value = []
    query.value = ''
    open.value = false
    error.value = ''
    activeIndex.value = -1
    onSelected(result)
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      open.value = false
      return
    }
    if (!results.value.length) return
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const delta = event.key === 'ArrowDown' ? 1 : -1
      const length = results.value.length
      activeIndex.value = (activeIndex.value + delta + length) % length
    } else if (event.key === 'Enter' && open.value) {
      event.preventDefault()
      const result = results.value[activeIndex.value]
      if (result) select(result)
    }
  }

  function openSearch(event: FocusEvent) {
    if (closeTimer) clearTimeout(closeTimer)
    if (event.target instanceof HTMLInputElement) event.target.select()
    if (results.value.length || error.value) {
      open.value = true
    }
  }

  function closeSearch() {
    closeTimer = setTimeout(() => {
      open.value = false
      closeTimer = null
    }, CLOSE_DELAY_MS)
  }

  function dispose() {
    if (searchTimer) clearTimeout(searchTimer)
    if (closeTimer) clearTimeout(closeTimer)
    searchTimer = null
    closeTimer = null
    request += 1
  }

  return {
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
  }
}
