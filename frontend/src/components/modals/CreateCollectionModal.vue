<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>创建收藏夹</h3>
      <div class="create-collection-form">
        <input
          type="text"
          :model-value="modelValue"
          @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
          placeholder="收藏夹名称"
          class="collection-input-full"
          @keyup.enter="$emit('confirm')"
          ref="nameInput"
        />
      </div>
      <div class="modal-actions">
        <button @click="$emit('close')" class="cancel-btn">取消</button>
        <button @click="$emit('confirm')" class="confirm-btn">确定</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'

defineProps<{
  modelValue: string
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'confirm'): void
  (e: 'update:modelValue', value: string): void
}>()

const nameInput = ref<HTMLInputElement | null>(null)

onMounted(() => {
  // Auto-focus the input when modal opens
  nameInput.value?.focus()
})
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0,0,0,0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-content {
  background-color: #fff;
  padding: 25px;
  border-radius: 10px;
  width: 90%;
  max-width: 400px;
  box-shadow: 0 4px 20px rgba(0,0,0,0.15);
}

.modal-content h3 {
  text-align: center;
  margin-bottom: 25px;
  font-size: 22px;
  font-weight: bold;
  color: #333;
  padding-bottom: 15px;
  border-bottom: 1px solid #eee;
}

.create-collection-form {
  margin-bottom: 20px;
}

.collection-input-full {
  width: 100%;
  padding: 12px 15px;
  border: 1px solid #ddd;
  border-radius: 5px;
  font-size: 16px;
  outline: none;
  box-sizing: border-box;
  transition: border-color 0.2s;
}

.collection-input-full:focus {
  border-color: #1db954;
}

.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
}

.confirm-btn, .cancel-btn {
  padding: 10px 20px;
  border: none;
  border-radius: 5px;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s;
}

.confirm-btn {
  background-color: #1db954;
  color: white;
}

.confirm-btn:hover {
  background-color: #1ed760;
}

.cancel-btn {
  background-color: #f0f0f0;
  color: #555;
}

.cancel-btn:hover {
  background-color: #e5e5e5;
}
</style>
