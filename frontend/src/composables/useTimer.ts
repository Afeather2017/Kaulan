import { ref, computed, watch, onUnmounted } from 'vue'

export function useTimer(onTimerComplete?: () => void) {
  // State
  const timerMinutes = ref(30)
  const timerMinutesInput = ref(30)
  const timerActive = ref(false)
  const timerRemaining = ref(0)
  const timerInterval = ref<number | null>(null)

  const timerStatusDisplay = computed(() => {
    if (timerActive.value) {
      const minutes = Math.floor(timerRemaining.value / 60)
      const seconds = timerRemaining.value % 60
      return `已定时: ${minutes}分${seconds.toString().padStart(2, '0')}秒后停止`
    }
    return '未启用定时'
  })

  const setTimerPreset = (minutes: number) => {
    timerMinutes.value = minutes
    timerMinutesInput.value = minutes
    startTimer()
  }

  const startTimer = () => {
    if (timerMinutes.value > 0) {
      // Clear existing interval
      if (timerInterval.value) {
        clearInterval(timerInterval.value)
      }

      timerActive.value = true
      timerRemaining.value = timerMinutes.value * 60

      timerInterval.value = window.setInterval(() => {
        timerRemaining.value--

        if (timerRemaining.value <= 0) {
          cancelTimer()
          onTimerComplete?.()
        }
      }, 1000)
    }
  }

  const cancelTimer = () => {
    if (timerInterval.value) {
      clearInterval(timerInterval.value)
      timerInterval.value = null
    }
    timerActive.value = false
    timerRemaining.value = 0
  }

  // Sync slider and input for timer
  watch(timerMinutes, (val) => {
    timerMinutesInput.value = val
  })

  // Cleanup
  onUnmounted(() => {
    if (timerInterval.value) {
      clearInterval(timerInterval.value)
    }
  })

  return {
    // State
    timerMinutes,
    timerMinutesInput,
    timerActive,
    timerRemaining,
    timerStatusDisplay,
    // Methods
    setTimerPreset,
    startTimer,
    cancelTimer
  }
}
