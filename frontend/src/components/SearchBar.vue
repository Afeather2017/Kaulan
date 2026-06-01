<template>
  <div class="search-bar">
    <input
      :value="modelValue"
      @input="
        $emit('update:modelValue', ($event.target as HTMLInputElement).value)
      "
      type="text"
      placeholder="搜索所有音乐来源..."
      class="search-input"
      name="library-search"
      @keyup.enter="$emit('search')"
    />
    <button class="search-button" @click="$emit('search')">搜索</button>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  modelValue: string;
}>();

defineEmits<{
  (e: "update:modelValue", value: string): void;
  (e: "search"): void;
}>();
</script>

<style scoped>
.search-bar {
  padding: 0;
  background-color: transparent;
  box-shadow: none;
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  flex: 1;
}

.search-input {
  flex: 1;
  padding: 10px 12px;
  border: 1px solid #ddd;
  border-radius: 10px 0 0 10px;
  font-size: 16px;
  outline: none;
  height: 36px;
  box-sizing: border-box;
}

.search-button {
  padding: 8px 12px;
  background-color: #1db954;
  color: white;
  border: none;
  border-radius: 0 10px 10px 0;
  cursor: pointer;
  font-weight: bold;
  font-size: 16px;
  height: 36px;
  box-sizing: border-box;
}

.search-button:hover {
  background-color: #1ed760;
}

.search-input:focus {
  border-color: #1db954;
}
</style>
