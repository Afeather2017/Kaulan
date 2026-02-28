# Examples

## Example 1: Settings Panel (Left Slide)

```
User: /tauri-create-popup-window Settings left "播放器设置"

[Creates SettingsModal.vue with:]
- Slides in from left (translateX -100% to 0)
- Max width 500px
- Full height
- Background #fafafa
- Border-right instead of border-top
```

## Example 2: Upload Panel (Bottom Slide)

```
User: /tauri-create-popup-window Upload bottom "上传文件"

[Creates UploadModal.vue with:]
- Slides up from bottom (translateY 100% to 0)
- Full width
- Top 30vh transparent
- Bottom 70vh white content
- Border-top separating areas
```

## Example 3: Playlist Panel (Bottom Slide)

```
User: /tauri-create-popup-window Playlist bottom "当前播放列表"

[Creates PlaylistModal.vue with:]
- Song list display
- Active song highlighting
- Click to play functionality
- Back button to close
```

## File Output Structure

Each generated panel follows this structure:

```
frontend/src/components/modals/[Name]Modal.vue
├── <template>
│   ├── .[name]-panel (outer container)
│   │   ├── .panel-transparent-top (for bottom panels)
│   │   └── .[name]-panel-content
│   │       ├── .panel-top-bar
│   │       │   ├── .top-back-btn
│   │       │   └── .panel-title
│   │       └── .panel-body
│   │           └── [content]
├── <script setup lang="ts">
│   ├── Props definition
│   └── Emits definition
└── <style scoped>
    ├── Animation keyframes
    ├── Panel positioning
    └── Component styles
```
