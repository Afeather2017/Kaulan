<template>
  <!--
    Shared "← 返回" control used by content panels (see
    `docs/frontend/app-shell-architecture.md` — "Keep view actions near the
    view"). Emits `back`; the parent wires it to the stack-backed
    `handleActionBack` handler.
  -->
  <button class="back-button" @click="$emit('back')">← 返回</button>
</template>

<script setup lang="ts">
// Related documentation: `docs/frontend/app-shell-architecture.md`

defineEmits<{
  (e: "back"): void;
}>();
</script>

<style scoped>
.back-button {
  background: none;
  border: none;
  color: #1db954;
  font-size: 14px;
  cursor: pointer;
  padding: 5px;
  white-space: nowrap;
}
</style>
