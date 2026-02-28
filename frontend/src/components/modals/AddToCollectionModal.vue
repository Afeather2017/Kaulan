<template>
  <div class="addto-panel">
    <div class="panel-transparent-top"></div>
    <div class="addto-panel-content">
      <div class="panel-top-bar">
        <button class="top-back-btn" @click="$emit('close')">
          <i class="fas fa-arrow-left"></i>
          返回
        </button>
        <h3 class="panel-title">添加到收藏夹</h3>
      </div>
      <div class="panel-body">
        <div class="collection-select-list">
          <div
            v-for="collection in collections.filter(c => c.name !== '所有音乐')"
            :key="collection.id"
            class="collection-checkbox-item"
          >
            <input
              type="checkbox"
              :id="'collection-' + collection.id"
              :value="collection.id"
              :checked="selectedCollectionIds.includes(collection.id)"
              @change="$emit('toggleSelection', collection.id)"
            />
            <label :for="'collection-' + collection.id">{{ collection.name }}</label>
          </div>
        </div>
        <div class="panel-actions">
          <button @click="$emit('close')" class="cancel-btn">取消</button>
          <button @click="$emit('confirm')" class="confirm-btn">确定</button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
export interface Collection {
  id: number
  name: string
  created_at: string
}

defineProps<{
  collections: Collection[]
  selectedCollectionIds: number[]
}>()

defineEmits<{
  (e: 'close'): void
  (e: 'confirm'): void
  (e: 'toggleSelection', id: number): void
}>()
</script>

<style scoped>
.addto-panel {
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

@keyframes slideIn {
  from {
    transform: translateY(100%);
  }
  to {
    transform: translateY(0);
  }
}

.addto-panel-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  background-color: #fff;
  border-top: 1px solid #eee;
}

.panel-transparent-top {
  flex: none;
  height: 30vh;
  background-color: transparent;
  pointer-events: none;
}

.panel-top-bar {
  flex: none;
  padding: 12px 20px;
  border-bottom: 1px solid #eee;
  display: flex;
  align-items: center;
  gap: 12px;
  background-color: #fff;
}

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

.top-back-btn:hover {
  background-color: #f0f0f0;
  border-color: #ccc;
}

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

.panel-body {
  flex: 1;
  padding: 20px;
  overflow-y: auto;
}

.collection-select-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.collection-checkbox-item {
  display: flex;
  align-items: center;
  padding: 12px 15px;
  border-bottom: 1px solid #f0f0f0;
  cursor: pointer;
  transition: background-color 0.2s;
}

.collection-checkbox-item:hover {
  background-color: #f9f9f9;
}

.collection-checkbox-item input[type="checkbox"] {
  width: 20px;
  height: 20px;
  margin-right: 10px;
  cursor: pointer;
}

.collection-checkbox-item label {
  cursor: pointer;
  flex: 1;
  font-size: 16px;
}

.panel-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
  margin-top: 25px;
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
