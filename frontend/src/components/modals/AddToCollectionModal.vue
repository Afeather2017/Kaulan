<template>
  <div class="modal-overlay" @click="$emit('close')">
    <div class="modal-content" @click.stop>
      <h3>添加到收藏夹</h3>
      <div class="collection-select-list">
        <div
          v-for="collection in collections.filter((c) => c.name !== '所有音乐')"
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
          <label :for="'collection-' + collection.id">{{
            collection.name
          }}</label>
        </div>
      </div>
      <div class="modal-actions">
        <button @click="$emit('close')" class="cancel-btn">取消</button>
        <button @click="$emit('confirm')" class="confirm-btn">确定</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
export interface Collection {
  id: number;
  name: string;
  created_at: string;
}

defineProps<{
  collections: Collection[];
  selectedCollectionIds: number[];
}>();

defineEmits<{
  (e: "close"): void;
  (e: "confirm"): void;
  (e: "toggleSelection", id: number): void;
}>();
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background-color: rgba(0, 0, 0, 0.5);
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
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  max-height: 80vh;
  overflow-y: auto;
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

.collection-select-list {
  max-height: 300px;
  overflow-y: auto;
  margin-bottom: 20px;
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

.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: center;
}

.confirm-btn,
.cancel-btn {
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
