# Pinch-to-Zoom Prevention

## Overview

By default, Android WebView allows pinch-to-zoom gestures, which can cause unintended scaling of the web interface. This document describes how Kaulan prevents zooming on the Android platform while maintaining proper touch interactions.

## Problem

On Android, the WebView component may respond to pinch gestures with two fingers, causing the entire page to scale. This behavior is undesirable for a native app-like music player experience where the UI should remain fixed at the designed scale.

## Solution

The solution uses two complementary approaches:

1. **Viewport meta tag** - Browser-level prevention of scaling
2. **CSS `touch-action` property** - Runtime touch behavior control

### Viewport Meta Tag

The `index.html` file contains a viewport meta tag that prevents scaling at the browser level:

```html
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no" />
```

This tells the browser to:
- Set the viewport width to match the device width
- Lock the initial scale to 1.0
- Prevent scaling beyond 1.0 (`maximum-scale=1`)
- Disable user-initiated zoom gestures (`user-scalable=no`)

### CSS touch-action

When the app detects it's running on Android, it applies additional styles that:

1. Disable pinch-to-zoom gestures
2. Allow only pan-x and pan-y (horizontal/vertical scrolling)
3. Prevent body overflow scrolling

## Implementation

### Viewport Meta Tag

**`frontend/index.html`** - HTML entry point with viewport configuration

```html
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1, user-scalable=no" />
```

### CSS touch-action

**`frontend/src/main.ts`** - Entry point where Android-specific styles are applied

```typescript
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
```

### Platform Detection

**`frontend/src/utils/platform.ts`** - Platform detection utility

The `isAndroid()` function uses cached user agent detection to identify the Android platform:

```typescript
export function isAndroid(): boolean {
  if (cachedIsAndroid !== null) {
    return cachedIsAndroid
  }
  cachedIsAndroid = /android/i.test(navigator.userAgent)
  return cachedIsAndroid
}
```

## How It Works

### Viewport Meta Tag

The viewport meta tag provides browser-level scaling prevention:

| Attribute | Purpose |
|-----------|---------|
| `width=device-width` | Match viewport to device screen width |
| `initial-scale=1` | Set initial zoom level to 1.0 |
| `maximum-scale=1` | Prevent zooming in beyond 1.0 |
| `user-scalable=no` | Disable user-initiated zoom gestures |

This is the first line of defense and works across all platforms.

### touch-action Property

The `touch-action` CSS property controls how touch interactions are handled:

| Value | Behavior |
|-------|----------|
| `pan-x pan-y` | Allow horizontal and vertical panning only, disable zoom |
| `auto` | Default browser behavior (allows zoom) |
| `none` | Disable all touch interactions |

By setting `touch-action: pan-x pan-y`, we explicitly allow scrolling but disable pinch-to-zoom.

### Style Application Timing

The styles are applied in `main.ts` before mounting the Vue app, ensuring they take effect before any component rendering. This prevents initial layout shifts and zooming issues.

## Related Source Files

- **`frontend/index.html`** - Viewport meta tag configuration
- **`frontend/src/main.ts`** - Entry point, applies Android touch styles
- **`frontend/src/utils/platform.ts`** - Platform detection (`isAndroid()` function)

## Troubleshooting

### Zooming Still Works

If pinch-to-zoom still works on Android:

1. **Check platform detection** - Verify `isAndroid()` returns `true`:
   ```typescript
   console.log(isAndroid()) // Should be true
   ```

2. **Verify styles are applied** - Check DevTools:
   ```javascript
   console.log(document.body.style.touchAction) // Should show "pan-x pan-y"
   ```

3. **Check for conflicting styles** - Search for other `touch-action` declarations in CSS files

### Scrolling Issues

If you need scrolling in specific components:

1. The `overflow: hidden` on body prevents page-level scrolling
2. Individual components can still use `overflow: auto` or `overflow: scroll` for internal scrolling
3. Consider using Vue `<scroll-view>` or similar components for scrollable areas

## Testing

To verify the fix works:

1. Build the Android app: `cd frontend && npm run tauri android build`
2. Install and run on a physical Android device (emulator may not support multi-touch)
3. Try pinching with two fingers - the page should not scale
4. Verify normal touch interactions (tapping, swiping) still work correctly

## See Also

- **[`tasks.md`](../../tasks.md)** - Task #1: "Don't allow user to scale the window"
- **[`android-secure-config.md`](./android-secure-config.md)** - Android build configuration
