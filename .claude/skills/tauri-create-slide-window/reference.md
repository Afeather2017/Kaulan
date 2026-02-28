# Reference: Tauri Slide-in Panel Components

## Architecture

Slide-in panels replace traditional modal overlays with a more mobile-friendly approach. They provide better UX on touch devices and follow modern mobile app patterns.

## Key Design Decisions

### Why Slide-in Panels?

1. **Mobile-first UX** - Natural touch interaction pattern
2. **Better space utilization** - Panels don't block entire screen
3. **Transparent top areas** - Users can see app context behind
4. **No overlay backdrop** - Cleaner, more modern appearance
5. **Consistent navigation** - Back button behavior matches mobile OS conventions

### Panel Categories

| Category | Slide Direction | Typical Content | Examples |
|----------|----------------|-----------------|----------|
| Navigation | Left (→) | Settings, menus, filters | Settings modal |
| Content | Bottom (↑) | Forms, lists, details | Upload, Playlist, AddToCollection |
| Quick Action | Bottom (↑) | Confirmations, small inputs | CreateCollection |

## CSS Properties Reference

### Left Panel (Navigation)
```css
[name]-panel {
  position: fixed;
  top: 0;
  left: 0;
  bottom: 0;
  width: 100%;
  max-width: 500px;
  background-color: #fafafa;
  border-right: 1px solid #eee;
  z-index: 100;
  animation: slideIn 0.3s ease-out;
  display: flex;
  flex-direction: column;
}

@keyframes slideIn {
  from { transform: translateX(-100%); }
  to { transform: translateX(0); }
}
```

### Bottom Panel (Content)
```css
[name]-panel {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  width: 100%;
  background-color: transparent;
  z-index: 100;
  animation: slideIn 0.3s ease-out;
  display: flex;
  flex-direction: column;
}

.panel-transparent-top {
  flex: none;
  height: 30vh;
  background-color: transparent;
  pointer-events: none;
}

[name]-panel-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  background-color: #fff;
  border-top: 1px solid #eee;
}

@keyframes slideIn {
  from { transform: translateY(100%); }
  to { transform: translateY(0); }
}
```

## Common Components

### panel-top-bar
```css
.panel-top-bar {
  flex: none;
  padding: 12px 20px;
  border-bottom: 1px solid #eee;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: #fff;
}
```

### top-back-btn
```css
.top-back-btn {
  border: 1px solid #ddd;
  background-color: #f8f8f8;
  color: #333;
  font-size: 15px;
  font-weight: 500;
  display: flex;
  align-items: center;
  gap: 6px;
  cursor: pointer;
  border-radius: 999px;
  padding: 6px 12px;
  transition: all 0.2s;
}
```

### panel-title
```css
.panel-title {
  margin: 0;
  flex: 1;
  font-size: 18px;
  font-weight: 600;
  color: #333;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
```

### panel-body
```css
.panel-body {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}
```

## Implementation Files

### Location
`frontend/src/components/modals/[Name]Modal.vue`

### Required Props/Emits
```typescript
// Props
defineProps<{
  // ... panel-specific props
}>()

// Emits
defineEmits<{
  (e: 'close'): void
  // ... panel-specific events
}>()
```

## Integration with App.vue

```vue
<!-- In App.vue -->
<template>
  <div class="app-window">
    <!-- Main content -->
  </div>

  <!-- Panel Component -->
  <SettingsModal
    v-if="showSettings"
    :prop="value"
    @close="showSettings = false"
    @event="handleEvent"
  />
</template>

<script setup lang="ts">
import SettingsModal from '@/components/modals/SettingsModal.vue'

const showSettings = ref(false)
</script>
```

## Migration from Modal Overlay

### Before (Modal Overlay)
```vue
<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <!-- content -->
    </div>
  </div>
</template>

<style>
.modal-overlay {
  position: fixed;
  background-color: rgba(0,0,0,0.5);
  /* ... */
}
.modal-content {
  /* centered box */
}
</style>
```

### After (Slide-in Panel)
```vue
<template>
  <div class="panel">
    <div class="panel-transparent-top"></div>
    <div class="panel-content">
      <div class="panel-top-bar">...</div>
      <div class="panel-body"><!-- content --></div>
    </div>
  </div>
</template>

<style>
.panel {
  /* slide-in animation */
  /* transparent background */
}
.panel-content {
  /* white background */
  /* slide animation */
}
</style>
```
