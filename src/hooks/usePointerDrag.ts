import { ref } from 'vue'

interface PointerDragState {
  pointerId: number
  sourceIndex: number
  targetIndex: number
  startX: number
  startY: number
  offsetX: number
  offsetY: number
  card: HTMLElement
  handle: HTMLElement
}

const DRAG_THRESHOLD_PX = 5

/**
 * 指针拖拽排序组合式函数：拖动阈值、克隆幽灵卡片、命中检测。
 * 拖拽落点确定后回调 onReorder(from, to)，数据变更由调用方负责。
 */
export function usePointerDrag(onReorder: (from: number, to: number) => void) {
  const draggingIndex = ref<number | null>(null)
  const dropTargetIndex = ref<number | null>(null)

  let state: PointerDragState | null = null
  let ghost: HTMLElement | null = null

  function createGhost(current: PointerDragState) {
    const cloned = current.card.cloneNode(true) as HTMLElement
    cloned.classList.remove('is-dragging', 'is-drop-target')
    cloned.classList.add('drag-ghost')
    cloned.removeAttribute('data-drag-index')
    cloned.removeAttribute('data-testid')
    cloned.setAttribute('aria-hidden', 'true')
    cloned.querySelectorAll('[data-testid]').forEach((element) => {
      element.removeAttribute('data-testid')
    })

    const bounds = current.card.getBoundingClientRect()
    cloned.style.width = `${bounds.width}px`
    cloned.style.height = `${bounds.height}px`
    cloned.style.left = `${bounds.left}px`
    cloned.style.top = `${bounds.top}px`
    document.body.append(cloned)
    document.body.classList.add('is-reordering')
    ghost = cloned
  }

  function moveGhost(current: PointerDragState, clientX: number, clientY: number) {
    if (!ghost) return
    ghost.style.left = `${clientX - current.offsetX}px`
    ghost.style.top = `${clientY - current.offsetY}px`
  }

  function dropTargetAt(
    current: PointerDragState,
    clientX: number,
    clientY: number,
  ) {
    const hit = document
      .elementFromPoint?.(clientX, clientY)
      ?.closest<HTMLElement>('[data-drag-index]')
    if (hit && current.card.parentElement?.contains(hit)) {
      return Number(hit.dataset.dragIndex)
    }

    const cards = Array.from(
      current.card.parentElement?.querySelectorAll<HTMLElement>(
        '[data-drag-index]',
      ) ?? [],
    )
    const candidate = cards
      .map((card) => ({
        index: Number(card.dataset.dragIndex),
        bounds: card.getBoundingClientRect(),
      }))
      .filter(({ bounds }) => clientX >= bounds.left && clientX <= bounds.right)
      .sort(
        (left, right) =>
          Math.abs(clientY - (left.bounds.top + left.bounds.height / 2)) -
          Math.abs(clientY - (right.bounds.top + right.bounds.height / 2)),
      )[0]

    return candidate?.index ?? current.sourceIndex
  }

  function cleanup() {
    if (state?.handle.hasPointerCapture?.(state.pointerId)) {
      state.handle.releasePointerCapture(state.pointerId)
    }
    ghost?.remove()
    ghost = null
    document.body.classList.remove('is-reordering')
    state = null
    draggingIndex.value = null
    dropTargetIndex.value = null
  }

  function begin(index: number, event: PointerEvent) {
    if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) {
      return
    }
    const card = event.currentTarget.closest<HTMLElement>('[data-drag-index]')
    if (!card) return

    event.preventDefault()
    event.currentTarget.setPointerCapture?.(event.pointerId)
    const bounds = card.getBoundingClientRect()
    state = {
      pointerId: event.pointerId,
      sourceIndex: index,
      targetIndex: index,
      startX: event.clientX,
      startY: event.clientY,
      offsetX: event.clientX - bounds.left,
      offsetY: event.clientY - bounds.top,
      card,
      handle: event.currentTarget,
    }
  }

  function move(event: PointerEvent) {
    const current = state
    if (!current || current.pointerId !== event.pointerId) return

    if (draggingIndex.value === null) {
      const distance = Math.hypot(
        event.clientX - current.startX,
        event.clientY - current.startY,
      )
      if (distance < DRAG_THRESHOLD_PX) return

      draggingIndex.value = current.sourceIndex
      createGhost(current)
    }

    event.preventDefault()
    moveGhost(current, event.clientX, event.clientY)
    current.targetIndex = dropTargetAt(current, event.clientX, event.clientY)
    dropTargetIndex.value =
      current.targetIndex === current.sourceIndex ? null : current.targetIndex
  }

  function finish(event: PointerEvent) {
    const current = state
    if (!current || current.pointerId !== event.pointerId) return
    const didDrag = draggingIndex.value !== null

    if (didDrag) {
      event.preventDefault()
      current.targetIndex = dropTargetAt(current, event.clientX, event.clientY)
    }

    const { sourceIndex, targetIndex } = current
    cleanup()
    if (!didDrag || sourceIndex === targetIndex) return
    onReorder(sourceIndex, targetIndex)
  }

  function cancel(event: PointerEvent) {
    if (state && state.pointerId === event.pointerId) {
      cleanup()
    }
  }

  function dispose() {
    if (state || ghost) cleanup()
  }

  return {
    draggingIndex,
    dropTargetIndex,
    begin,
    move,
    finish,
    cancel,
    dispose,
  }
}
