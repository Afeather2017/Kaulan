<template>
  <div class="search-bar">
    <input
      :model-value="modelValue"
      @update:model-value="$emit('update:modelValue', $event)"
      type="text"
      placeholder="搜索歌曲或歌单"
      class="search-input"
      @keyup.enter="$emit('search')"
    />
    <button class="search-button" @click="$emit('search')">
      搜索
    </button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: string
}>()

defineEmits<{
  (e: 'update:modelValue', value: string): void
  (e: 'search'): void
}>()
</script>

<style scoped>
.search-bar {
  padding: 15px;
  background-color: #fff;
  box-shadow: 0 2px 5px rgba(0,0,0,0.1);
  position: relative;
  z-index: 10;
  display: flex;
  align-items: center;
}

.search-input {
  flex: 1;
  padding: 11px 15px;
  border: 1px solid #ddd;
  border-radius: 20px 0 0 20px;
  font-size: 16px;
  outline: none;
  height: 42px;
  box-sizing: border-box;
}

.search-button {
  padding: 11px 20px;
  background-color: #1db954;
  color: white;
  border: none;
  border-radius: 0 20px 20px 0;
  cursor: pointer;
  font-weight: bold;
  font-size: 16px;
  height: 42px;
  box-sizing: border-box;
}

.search-button:hover {
  background-color: #1ed760;
}

.search-input:focus {
  border-color: #1db954;
}
</style>
