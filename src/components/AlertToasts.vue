<script setup lang="ts">
import type { AlertToast } from '../settings'

defineProps<{ toasts: AlertToast[] }>()
const emit = defineEmits<{ dismiss: [key: number] }>()
</script>

<template>
  <!-- 整块做成 button：点哪都能关，键盘也能 Tab 到，不用另做一个小叉子的命中区 -->
  <div
    v-if="toasts.length"
    class="alert-toasts"
    data-testid="alert-toasts"
    aria-live="polite"
  >
    <button
      v-for="toast in toasts"
      :key="toast.key"
      type="button"
      class="alert-toast"
      :data-testid="`alert-toast-${toast.key}`"
      :aria-label="`关闭提醒：${toast.title}`"
      @click="emit('dismiss', toast.key)"
    >
      <strong class="alert-toast-title">{{ toast.title }}</strong>
      <span class="alert-toast-close" aria-hidden="true">×</span>
      <span class="alert-toast-body">{{ toast.body }}</span>
    </button>
  </div>
</template>
