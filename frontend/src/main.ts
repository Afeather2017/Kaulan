import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import { checkIsAndroid } from './utils/platform'
import '@fortawesome/fontawesome-free/css/all.css'

/**
 * Initialize the music notification service on Android
 * This starts the foreground service which keeps the backend alive
 */
async function initMusicService() {
  console.log('[MusicService] Starting music service initialization...')
  const isAndroid = await checkIsAndroid()
  console.log('[MusicService] Platform check result: isAndroid =', isAndroid)

  if (!isAndroid) {
    console.log('[MusicService] Not Android, skipping music service initialization')
    return
  }

  try {
    // Dynamically import the plugin API (only available on Android)
    console.log('[MusicService] Attempting to import music-notification-api...')
    const { startService } = await import('music-notification-api')
    console.log('[MusicService] ✓ Successfully imported music-notification-api')
    const startServiceResult = await startService()
    console.log('[MusicService] startService result:', startServiceResult)

    if (startServiceResult.success) {
      console.log('[MusicService] ✓ Music notification service started - backend will stay alive')
    } else {
      console.error('[MusicService] ✗ Failed to start music service:', startServiceResult.message)
    }
  } catch (e) {
    console.error('[MusicService] ✗ Failed to initialize music service:', e)
    console.warn('[MusicService] Music notification plugin not available (this is expected on desktop)')
  }
}

const startApp = async () => {
  // Initialize music service first (before Vue app)
  await initMusicService()

  // Apply Android-specific touch styles to prevent zooming and unwanted scrolling
  const isAndroid = await checkIsAndroid()
  if (isAndroid) {
    // Set root element touch behavior to prevent zooming
    document.documentElement.style.touchAction = 'pan-x pan-y'
    document.documentElement.style.height = '100%'

    // Set body styles to prevent scrolling
    document.body.style.touchAction = 'pan-x pan-y'
    document.body.style.overflow = 'hidden'
    document.body.style.height = '100%'
    document.body.style.margin = '0'
    document.body.style.padding = '0'
  }

  createApp(App).use(router).mount('#app')
}

startApp()
