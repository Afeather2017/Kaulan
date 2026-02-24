import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import { isAndroid } from './utils/platform'

// Apply Android-specific touch styles to prevent zooming and unwanted scrolling
if (isAndroid()) {
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