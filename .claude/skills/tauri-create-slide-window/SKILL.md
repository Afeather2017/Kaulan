---
name: tauri-create-popup-window
description: Create slide-in popup panels for Tauri/Vue applications with animations
disable-model-invocation: false
user-invocable: true
allowed-tools: "Read,Write,Edit,Bash"
context: inline
agent: general-purpose
---

# Tauri Create Popup Window

Create slide-in popup panels (modals) for Tauri/Vue applications. These panels slide in from left or bottom with transparent top areas, replacing traditional modal overlays.

## When to Use This Skill

Use this skill when you need to:
- Create slide-in panels for Tauri mobile apps
- Replace modal overlays with modern slide-in panels
- Create panels that slide from left (settings) or bottom (forms, lists)
- Implement panels with transparent top areas

## Panel Types

| Type | Direction | Width | Height | Use Case |
|------|-----------|-------|--------|----------|
| Left panel | From left (`translateX`) | max-width: 500px | 100vh | Settings, navigation |
| Bottom panel | From bottom (`translateY`) | 100% | 70vh | Forms, lists, popups |

## Structure

All panels use this consistent structure:

```vue
<template>
  <div class="[name]-panel">
    <div class="panel-transparent-top"></div>
    <div class="[name]-panel-content">
      <div class="panel-top-bar">
        <button class="top-back-btn" @click="$emit('close')">
          <i class="fas fa-arrow-left"></i>
          返回
        </button>
        <h3 class="panel-title">Title</h3>
      </div>
      <div class="panel-body">
        <!-- Content here -->
      </div>
    </div>
  </div>
</template>
```

## CSS Pattern

### Left Panel (Settings-style)
```css
.panel {
  position: fixed;
  top: 0; left: 0; bottom: 0;
  width: 100%;
  max-width: 500px;
  background-color: #fafafa;
  border-right: 1px solid #eee;
  animation: slideIn 0.3s ease-out;
}

@keyframes slideIn {
  from { transform: translateX(-100%); }
  to { transform: translateX(0); }
}
```

### Bottom Panel (Form-style)
```css
.panel {
  position: fixed;
  top: 0; left: 0; right: 0; bottom: 0;
  width: 100%;
  background-color: transparent;
  animation: slideIn 0.3s ease-out;
}

.panel-transparent-top {
  flex: none;
  height: 30vh;
  background-color: transparent;
  pointer-events: none;
}

.panel-content {
  flex: 1;
  background-color: #fff;
  border-top: 1px solid #eee;
}

@keyframes slideIn {
  from { transform: translateY(100%); }
  to { transform: translateY(0); }
}
```

## Usage

/tauri-create-popup-window [panel-name] [type] [title]

- `panel-name`: Name for the panel component (e.g., "Settings", "Upload")
- `type`: `left` or `bottom`
- `title`: Display title for the panel

## Examples

```
# Create a settings panel (slides from left)
/tauri-create-popup-window Settings left "播放器设置"

# Create a form panel (slides from bottom)
/tauri-create-popup-window Upload bottom "上传文件"

# Create a list panel (slides from bottom)
/tauri-create-popup-window Playlist bottom "当前播放列表"
```

## Implementation Notes

1. **No overlay background** - Panels don't use semi-transparent backdrops
2. **Transparent top area** - Bottom panels have 30vh transparent top for seeing app behind
3. **Flat design** - No border-radius or box-shadow on panels
4. **Pointer events** - Transparent areas have `pointer-events: none` to allow clicks through
5. **Consistent structure** - All panels use `panel-top-bar`, `panel-title`, `panel-body`
