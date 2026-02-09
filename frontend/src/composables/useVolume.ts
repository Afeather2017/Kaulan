import { ref, watch, type Ref } from 'vue'

export type VolumeMode = 'auto' | 'manual' | 'fixed'

export interface MusicInfo {
  name: string
  lufs: number
  path: string
}

const volumeModeLabels: Record<VolumeMode, string> = {
  auto: '自动音量平衡',
  manual: '手动设置音量',
  fixed: '固定音量大小'
}

export function useVolume(currentSong: Ref<MusicInfo | null>, currentSongs: Ref<MusicInfo[]>) {
  // State
  const volumeMode = ref<VolumeMode>('auto')
  const manualVolume = ref(0.5)
  const manualVolumeInput = ref(0.5)
  const fixedLufs = ref(-27)
  const fixedLufsInput = ref(-27)

  // Calculate volume based on current mode
  const calculateVolume = (): number => {
    if (!currentSong.value) return 0.5

    const song = currentSong.value

    if (volumeMode.value === 'auto') {
      // Find minimum LUFS in current playlist
      let minLufs = 1000
      for (const s of currentSongs.value) {
        minLufs = Math.min(s.lufs, minLufs)
      }
      return 10 ** ((minLufs - song.lufs) / 20)
    } else if (volumeMode.value === 'fixed') {
      return 10 ** ((fixedLufs.value - song.lufs) / 20)
    } else {
      return manualVolume.value
    }
  }

  const toggleVolumeMode = () => {
    const modes: VolumeMode[] = ['auto', 'manual', 'fixed']
    const currentIndex = modes.indexOf(volumeMode.value)
    volumeMode.value = modes[(currentIndex + 1) % modes.length]
  }

  // Sync slider and input bidirectionally for manual volume
  watch(manualVolume, (val) => {
    manualVolumeInput.value = val
  })

  watch(manualVolumeInput, (val) => {
    manualVolume.value = val
  })

  watch(fixedLufs, (val) => {
    fixedLufsInput.value = val
  })

  watch(fixedLufsInput, (val) => {
    fixedLufs.value = val
  })

  return {
    // State
    volumeMode,
    manualVolume,
    manualVolumeInput,
    fixedLufs,
    fixedLufsInput,
    volumeModeLabels,
    // Methods
    calculateVolume,
    toggleVolumeMode
  }
}
