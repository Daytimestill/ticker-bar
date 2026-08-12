// @vitest-environment jsdom

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useStockSearch } from './useStockSearch'

const invoke = vi.fn()

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}))

const XIAOMI = {
  symbol: '01810.HK',
  name: '小米集团-W',
  market: '港股',
  currency: 'HKD',
}
const INSPUR = {
  symbol: '600756.SH',
  name: '浪潮软件',
  market: '沪市',
  currency: 'CNY',
}

function setup() {
  const onSelected = vi.fn()
  const search = useStockSearch(onSelected)
  return { onSelected, search }
}

describe('useStockSearch', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    invoke.mockReset()
    invoke.mockResolvedValue([XIAOMI, INSPUR])
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('debounces queries and fills results with the first one active', async () => {
    const { search } = setup()

    search.query.value = '小米'
    search.schedule()
    expect(invoke).not.toHaveBeenCalled()

    await vi.advanceTimersByTimeAsync(300)

    expect(invoke).toHaveBeenCalledWith('search_stocks', { query: '小米' })
    expect(search.results.value).toEqual([XIAOMI, INSPUR])
    expect(search.activeIndex.value).toBe(0)
    expect(search.open.value).toBe(true)
  })

  it('clears state without a request when the query is emptied', async () => {
    const { search } = setup()

    search.query.value = '  '
    search.schedule()
    await vi.advanceTimersByTimeAsync(300)

    expect(invoke).not.toHaveBeenCalled()
    expect(search.open.value).toBe(false)
    expect(search.results.value).toEqual([])
  })

  it('navigates results with arrow keys and selects with enter', async () => {
    const { onSelected, search } = setup()
    search.query.value = '软件'
    search.schedule()
    await vi.advanceTimersByTimeAsync(300)

    search.handleKeydown(new KeyboardEvent('keydown', { key: 'ArrowDown' }))
    expect(search.activeIndex.value).toBe(1)
    search.handleKeydown(new KeyboardEvent('keydown', { key: 'ArrowDown' }))
    expect(search.activeIndex.value).toBe(0)
    search.handleKeydown(new KeyboardEvent('keydown', { key: 'ArrowUp' }))
    expect(search.activeIndex.value).toBe(1)

    search.handleKeydown(new KeyboardEvent('keydown', { key: 'Enter' }))

    expect(onSelected).toHaveBeenCalledExactlyOnceWith(INSPUR)
    expect(search.query.value).toBe('')
    expect(search.open.value).toBe(false)
  })

  it('closes the popover with escape', async () => {
    const { search } = setup()
    search.query.value = '小米'
    search.schedule()
    await vi.advanceTimersByTimeAsync(300)

    search.handleKeydown(new KeyboardEvent('keydown', { key: 'Escape' }))

    expect(search.open.value).toBe(false)
  })

  it('surfaces search failures without an Error: prefix', async () => {
    invoke.mockRejectedValue(new Error('行情服务不可用'))
    const { search } = setup()

    search.query.value = '小米'
    search.schedule()
    await vi.advanceTimersByTimeAsync(300)

    expect(search.error.value).toBe('行情服务不可用')
    expect(search.results.value).toEqual([])
    expect(search.open.value).toBe(true)
  })

  it('reopens on focus only when there is something to show, and closes after blur delay', async () => {
    const { search } = setup()
    search.query.value = '小米'
    search.schedule()
    await vi.advanceTimersByTimeAsync(300)

    search.closeSearch()
    await vi.advanceTimersByTimeAsync(200)
    expect(search.open.value).toBe(false)

    search.openSearch(new FocusEvent('focus'))
    expect(search.open.value).toBe(true)

    search.dispose()
    search.results.value = []
    search.error.value = ''
    search.open.value = false
    search.openSearch(new FocusEvent('focus'))
    expect(search.open.value).toBe(false)
  })

  it('ignores stale responses after dispose', async () => {
    const { search } = setup()
    search.query.value = '小米'
    search.schedule()
    search.dispose()
    await vi.advanceTimersByTimeAsync(300)

    expect(invoke).not.toHaveBeenCalled()
    expect(search.results.value).toEqual([])
  })
})
