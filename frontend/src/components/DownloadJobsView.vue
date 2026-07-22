<template>
  <div class="download-jobs-view">
    <div v-if="jobs.length === 0" class="empty-state">
      <div>当前没有正在进行的下载任务。</div>
    </div>

    <div v-else class="job-list">
      <article
        v-for="job in jobs"
        :key="job.key"
        :class="['job-card', `job-${job.snapshot.state}`]"
      >
        <div class="job-header">
          <div class="job-title">{{ job.title }}</div>
          <div class="job-header-meta">
            <div class="job-source">{{ job.snapshot.source }}</div>
            <button
              v-if="isTerminal(job.snapshot)"
              class="job-dismiss"
              type="button"
              aria-label="移除下载记录"
              @click="downloadsStore.dismissJob(job.key)"
            >
              ×
            </button>
          </div>
        </div>
        <div class="job-phase-row">
          <span class="job-phase">{{ phaseLabel(job.snapshot.phase) }}</span>
          <span v-if="job.snapshot.percent !== null" class="job-percent">
            {{ job.snapshot.percent }}%
          </span>
        </div>
        <div class="job-progress-track">
          <div
            class="job-progress-bar"
            :style="{ width: `${progressWidth(job.snapshot)}%` }"
          ></div>
        </div>
        <div class="job-message">{{ job.snapshot.message }}</div>
        <div v-if="job.snapshot.detail" class="job-detail">
          {{ job.snapshot.detail }}
        </div>
        <div v-if="job.snapshot.state === 'completed'" class="job-refresh-note">
          下载已完成，请刷新曲库后查看新歌曲。
        </div>
        <div v-if="job.snapshot.error" class="job-error">
          {{ job.snapshot.error }}
        </div>
      </article>
    </div>
  </div>
</template>

<script setup lang="ts">
import type {
  ActiveDownloadJob,
  DownloadJobSnapshot,
  DownloadPhase,
} from "@/stores/downloads";
import { useDownloadsStore } from "@/stores/downloads";

defineProps<{
  jobs: ActiveDownloadJob[];
}>();

const downloadsStore = useDownloadsStore();

const isTerminal = (snapshot: DownloadJobSnapshot): boolean =>
  snapshot.state === "completed" || snapshot.state === "failed";

const phaseLabel = (phase: DownloadPhase): string => {
  switch (phase) {
    case "queued":
      return "排队中";
    case "preparing":
      return "准备中";
    case "resolving_meta":
      return "获取信息";
    case "downloading":
      return "下载中";
    case "post_processing":
      return "处理音频";
    case "embedding_cover":
      return "写入封面";
    case "saving_lyrics":
      return "保存歌词";
    case "refreshing_library":
      return "刷新曲库";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
  }
};

const progressWidth = (snapshot: DownloadJobSnapshot): number => {
  if (snapshot.percent !== null) {
    return Math.max(4, snapshot.percent);
  }
  switch (snapshot.phase) {
    case "queued":
      return 6;
    case "preparing":
      return 12;
    case "resolving_meta":
      return 20;
    case "downloading":
      return 48;
    case "post_processing":
      return 72;
    case "embedding_cover":
      return 82;
    case "saving_lyrics":
      return 90;
    case "refreshing_library":
      return 96;
    case "completed":
      return 100;
    case "failed":
      return 100;
  }
};
</script>

<style scoped>
.download-jobs-view {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.empty-state {
  padding: 12px 0 20px;
  color: #60757e;
}

.job-list {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 4px 0 0;
}

.job-card {
  border: 1px solid #d6dde2;
  border-radius: 8px;
  padding: 14px;
  background: linear-gradient(180deg, #ffffff 0%, #f6f8f9 100%);
}

.job-card.job-completed {
  border-color: #b7d7c3;
}

.job-card.job-failed {
  border-color: #e2b8b8;
}

.job-header,
.job-phase-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}

.job-title {
  font-weight: 700;
  color: #17313b;
  min-width: 0;
  word-break: break-word;
}

.job-header-meta {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  gap: 8px;
}

.job-source {
  font-size: 12px;
  color: #5d7078;
  text-transform: capitalize;
}

.job-dismiss {
  width: 28px;
  height: 28px;
  border: 1px solid #cfd8dd;
  border-radius: 50%;
  background: #ffffff;
  color: #45606a;
  font-size: 18px;
  line-height: 1;
}

.job-phase-row {
  margin-top: 8px;
  font-size: 13px;
  color: #45606a;
}

.job-progress-track {
  margin-top: 10px;
  height: 8px;
  border-radius: 999px;
  background: #e6ecef;
  overflow: hidden;
}

.job-progress-bar {
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, #d4662f 0%, #f0a43b 100%);
  transition: width 0.25s ease;
}

.job-message {
  margin-top: 10px;
  color: #17313b;
  font-size: 14px;
}

.job-detail {
  margin-top: 6px;
  color: #60757e;
  font-size: 12px;
  word-break: break-word;
}

.job-refresh-note,
.job-error {
  margin-top: 8px;
  font-size: 13px;
  word-break: break-word;
}

.job-refresh-note {
  color: #237247;
}

.job-error {
  color: #a33f3f;
}
</style>
