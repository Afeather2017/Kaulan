import { ref } from 'vue'
import { getApiBase, setApiBase } from '@/utils/api'

export interface DiscoveredDevice {
  device_id: string
  device_name: string
  api_url: string
  last_seen_secs_ago: number
  isManual?: boolean
}

export interface SelfDevice {
  device_id: string
  device_name: string
}

export interface SetDeviceNameResponse {
  success: boolean
  message: string
}

interface OperationResponse {
  success: boolean
  message: string
}

const SCAN_SECONDS = 3
const SCAN_INTERVAL_MS = 1000

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => {
    setTimeout(resolve, ms)
  })
}

export function useDeviceDiscovery() {
  const devices = ref<DiscoveredDevice[]>([])
  const selfDevice = ref<SelfDevice | null>(null)
  const isLoading = ref(false)
  const error = ref<string | null>(null)

  /**
   * Fetch all discovered devices from the server.
   */
  const fetchDevices = async (): Promise<void> => {
    const response = await fetch(`${getApiBase()}/discovery/devices`)
    if (!response.ok) {
      throw new Error(`Failed to fetch devices: ${response.statusText}`)
    }
    devices.value = await response.json()
  }

  /**
   * Refresh devices using manual 10-second request scan.
   *
   * Flow:
   * 1. Start scan transaction
   * 2. Send discovery request every 1 second for 10 seconds
   * 3. Commit scan on success, rollback on failure
   * 4. Fetch committed device list
   */
  const refreshDevices = async (): Promise<void> => {
    isLoading.value = true
    error.value = null

    let scanStarted = false

    try {
      const startResponse = await fetch(`${getApiBase()}/discovery/scan/start`, {
        method: 'POST',
      })
      if (!startResponse.ok) {
        throw new Error(`Failed to start scan: ${startResponse.statusText}`)
      }
      scanStarted = true

      for (let i = 0; i < SCAN_SECONDS; i += 1) {
        const requestResponse = await fetch(`${getApiBase()}/discovery/request`, {
          method: 'POST',
        })

        if (!requestResponse.ok) {
          throw new Error(`Failed to send discovery request: ${requestResponse.statusText}`)
        }

        const requestResult: OperationResponse = await requestResponse.json()
        if (!requestResult.success) {
          throw new Error(requestResult.message)
        }

        if (i < SCAN_SECONDS - 1) {
          await sleep(SCAN_INTERVAL_MS)
        }
      }

      const finishResponse = await fetch(`${getApiBase()}/discovery/scan/finish`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ success: true }),
      })

      if (!finishResponse.ok) {
        throw new Error(`Failed to finish scan: ${finishResponse.statusText}`)
      }

      await fetchDevices()
    } catch (err) {
      error.value = err instanceof Error ? err.message : 'Unknown error'
      console.error('Failed to refresh discovered devices:', err)

      if (scanStarted) {
        try {
          await fetch(`${getApiBase()}/discovery/scan/finish`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ success: false }),
          })
        } catch (rollbackError) {
          console.error('Failed to rollback discovery scan:', rollbackError)
        }
      }
    } finally {
      isLoading.value = false
    }
  }

  /**
   * Fetch this device's information.
   */
  const fetchSelfDevice = async (): Promise<void> => {
    try {
      const response = await fetch(`${getApiBase()}/discovery/self`)
      if (!response.ok) {
        throw new Error(`Failed to fetch self device: ${response.statusText}`)
      }

      selfDevice.value = await response.json()
    } catch (err) {
      console.error('Failed to fetch self device info:', err)
    }
  }

  /**
   * Set this device's name.
   */
  const setDeviceName = async (name: string): Promise<boolean> => {
    try {
      const response = await fetch(`${getApiBase()}/discovery/name`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      })

      if (!response.ok) {
        throw new Error(`Failed to set device name: ${response.statusText}`)
      }

      const result: SetDeviceNameResponse = await response.json()
      if (result.success) {
        await fetchSelfDevice()
        return true
      }
      return false
    } catch (err) {
      console.error('Failed to set device name:', err)
      return false
    }
  }

  /**
   * Connect to a discovered device.
   */
  const connectToDevice = (device: DiscoveredDevice): void => {
    let apiUrl = device.api_url
    if (!apiUrl.endsWith('/api')) {
      apiUrl = apiUrl.endsWith('/') ? apiUrl + 'api' : apiUrl + '/api'
    }

    setApiBase(apiUrl)
    window.location.reload()
  }

  const formatLastSeen = (secs: number): string => {
    if (secs < 60) return `${secs}秒前`
    if (secs < 3600) return `${Math.floor(secs / 60)}分钟前`
    return `${Math.floor(secs / 3600)}小时前`
  }

  return {
    devices,
    selfDevice,
    isLoading,
    error,
    fetchDevices,
    refreshDevices,
    fetchSelfDevice,
    setDeviceName,
    connectToDevice,
    formatLastSeen,
  }
}
