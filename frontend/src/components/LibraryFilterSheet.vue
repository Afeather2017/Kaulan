<template>
  <div class="sheet-overlay" @click="$emit('close')">
    <div class="sheet-card" @click.stop>
      <div class="sheet-header">
        <div class="sheet-title">筛选曲库</div>
      </div>

      <div class="sheet-section">
        <div class="section-label">来源</div>
        <label class="filter-option">
          <input
            type="radio"
            name="library-source-filter"
            value="all"
            :checked="draftSourceKey === 'all'"
            @change="$emit('update:draftSourceKey', 'all')"
          />
          <span>全部来源</span>
        </label>
        <label
          v-for="source in sources"
          :key="source.sourceKey"
          class="filter-option"
        >
          <input
            type="radio"
            name="library-source-filter"
            :value="source.sourceKey"
            :checked="draftSourceKey === source.sourceKey"
            @change="$emit('update:draftSourceKey', source.sourceKey)"
          />
          <span>{{ source.name }}</span>
        </label>
      </div>

      <div class="sheet-section">
        <div class="section-label">类型</div>
        <label class="filter-option">
          <input
            type="checkbox"
            :checked="draftMediaTypes.includes('audio')"
            @change="
              $emit(
                'toggleMediaType',
                'audio',
                ($event.target as HTMLInputElement).checked,
              )
            "
          />
          <span>音频</span>
        </label>
        <label class="filter-option">
          <input
            type="checkbox"
            :checked="draftMediaTypes.includes('video')"
            @change="
              $emit(
                'toggleMediaType',
                'video',
                ($event.target as HTMLInputElement).checked,
              )
            "
          />
          <span>视频</span>
        </label>
      </div>

      <div class="sheet-actions">
        <button class="secondary-btn" @click="$emit('reset')">重置</button>
        <button class="primary-btn" @click="$emit('apply')">应用</button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
defineProps<{
  sources: Array<{ sourceKey: string; name: string }>;
  draftSourceKey: string;
  draftMediaTypes: string[];
}>();

defineEmits<{
  (e: "close"): void;
  (e: "apply"): void;
  (e: "reset"): void;
  (e: "update:draftSourceKey", value: string): void;
  (e: "toggleMediaType", type: "audio" | "video", enabled: boolean): void;
}>();
</script>

<style scoped>
.sheet-overlay {
  position: fixed;
  top: 0;
  right: 0;
  bottom: 0;
  left: 0;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: flex-end;
  justify-content: center;
  z-index: 130;
}

.sheet-card {
  width: min(100%, 560px);
  background: #fff;
  border-radius: 18px 18px 0 0;
  padding: 20px 18px 24px;
  box-shadow: 0 -12px 30px rgba(0, 0, 0, 0.12);
}

.sheet-header {
  margin-bottom: 14px;
}

.sheet-title {
  font-size: 20px;
  font-weight: 700;
  color: #223;
}

.sheet-section {
  padding: 12px 0;
  border-top: 1px solid #eef1f4;
}

.sheet-section:first-of-type {
  border-top: none;
}

.section-label {
  margin-bottom: 10px;
  font-size: 14px;
  font-weight: 700;
  color: #556372;
}

.filter-option {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
  color: #223;
}

.sheet-actions {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  margin-top: 18px;
}

.primary-btn,
.secondary-btn {
  flex: 1;
  border: none;
  border-radius: 10px;
  padding: 12px 14px;
  font-weight: 700;
  cursor: pointer;
}

.primary-btn {
  background: #1db954;
  color: #fff;
}

.secondary-btn {
  background: #edf1f5;
  color: #223;
}
</style>
