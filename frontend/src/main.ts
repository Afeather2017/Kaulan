import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import { getRuntimeProfile } from "./utils/platform";
import "@fortawesome/fontawesome-free/css/all.css";

/**
 * Initialize the music notification service on Android
 * This starts the foreground service which keeps the backend alive
 */
async function initMusicService() {
  const runtime = await getRuntimeProfile();
  if (!runtime.capabilities.supportsForegroundMusicService) {
    return;
  }

  try {
    const { startService } = await import("music-notification-api");
    const startServiceResult = await startService();

    if (!startServiceResult.success) {
      console.error(
        "[MusicService] ✗ Failed to start music service:",
        startServiceResult.message,
      );
    }
  } catch (e) {
    console.error("[MusicService] ✗ Failed to initialize music service:", e);
    console.warn(
      "[MusicService] Music notification plugin not available (this is expected on desktop)",
    );
  }
}

const startApp = async () => {
  const runtime = await getRuntimeProfile();
  await initMusicService();

  // Apply Android-specific touch styles to prevent zooming and unwanted scrolling
  if (runtime.capabilities.usesAndroidPlaybackBackend) {
    // Set root element touch behavior to prevent zooming
    document.documentElement.style.touchAction = "pan-x pan-y";
    document.documentElement.style.height = "100%";

    // Set body styles to prevent scrolling
    document.body.style.touchAction = "pan-x pan-y";
    document.body.style.overflow = "hidden";
    document.body.style.height = "100%";
    document.body.style.margin = "0";
    document.body.style.padding = "0";
  }

  const app = createApp(App);
  app.use(createPinia());
  app.mount("#app");
};

startApp();
