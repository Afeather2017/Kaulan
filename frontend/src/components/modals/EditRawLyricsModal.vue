<template>
  <div class="modal-overlay" @click.self="handleBackdropClick">
    <div class="modal-content" @click.stop>
      <div class="modal-header">
        <button
          type="button"
          class="header-btn"
          :disabled="isSubmitting"
          @click="handleCancel"
        >
          Cancel
        </button>
        <h3 class="header-title">Edit Lyrics</h3>
        <button
          type="button"
          class="header-btn primary"
          :disabled="isSubmitting || !canSubmit"
          @click="handleSubmit"
        >
          {{ isSubmitting ? "Saving..." : "Save" }}
        </button>
      </div>

      <p class="source-caption">
        Edit the raw lyric text. Keep the
        <code>[mm:ss.xx]</code> timestamps intact — the app does not reformat
        them for you.
      </p>

      <div v-if="submitError" class="modal-error">{{ submitError }}</div>

      <textarea
        ref="textareaRef"
        v-model="draft"
        class="lyric-textarea"
        spellcheck="false"
        autocapitalize="off"
        autocomplete="off"
        :disabled="isSubmitting"
        @input="autoresize"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
// Related documentation: `docs/lyric-editing.md`
import { computed, nextTick, ref, watch } from "vue";

const props = defineProps<{
  musicId: number | null;
  rawLyricsContent: string | null;
  visible: boolean;
  lyricApiBase: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "saved"): void;
}>();

const textareaRef = ref<HTMLTextAreaElement | null>(null);
const draft = ref("");
const isSubmitting = ref(false);
const submitError = ref<string | null>(null);

const canSubmit = computed(
  () =>
    draft.value.trim().length > 0 &&
    draft.value !== (props.rawLyricsContent ?? ""),
);

function resetDraftFromProps() {
  draft.value = props.rawLyricsContent ?? "";
  submitError.value = null;
  isSubmitting.value = false;
  nextTick(() => {
    autoresize();
    textareaRef.value?.focus();
  });
}

function autoresize() {
  const el = textareaRef.value;
  if (!el) {
    return;
  }
  el.style.height = "auto";
  el.style.height = `${el.scrollHeight}px`;
}

function handleBackdropClick() {
  handleCancel();
}

function handleCancel() {
  if (isSubmitting.value) {
    return;
  }
  if (
    draft.value !== (props.rawLyricsContent ?? "") &&
    typeof window !== "undefined" &&
    !window.confirm("Discard lyric edits?")
  ) {
    return;
  }
  emit("close");
}

async function handleSubmit() {
  if (!props.musicId || isSubmitting.value || !canSubmit.value) {
    return;
  }

  isSubmitting.value = true;
  submitError.value = null;

  try {
    const response = await fetch(
      `${props.lyricApiBase}/lyrics/id/${props.musicId}`,
      {
        method: "PUT",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ content: draft.value }),
      },
    );

    if (!response.ok) {
      const payload = (await response.json().catch(() => null)) as {
        message?: string;
      } | null;
      if (response.status === 409) {
        submitError.value =
          payload?.message ??
          "This source is read-only — lyrics can't be saved from this device.";
      } else {
        submitError.value =
          payload?.message || `Failed to save lyrics (${response.status})`;
      }
      return;
    }

    emit("saved");
  } catch (error) {
    console.error("Failed to save raw lyric content:", error);
    submitError.value = "Failed to save lyrics";
  } finally {
    isSubmitting.value = false;
  }
}

watch(
  () => props.visible,
  (visible) => {
    if (visible) {
      resetDraftFromProps();
    }
  },
  { immediate: true },
);

watch(
  () => props.rawLyricsContent,
  () => {
    if (props.visible) {
      resetDraftFromProps();
    }
  },
);
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
  padding: 16px;
  box-sizing: border-box;
}

.modal-content {
  background-color: #fff;
  border-radius: 12px;
  width: 100%;
  max-width: 640px;
  max-height: calc(100vh - 32px);
  display: flex;
  flex-direction: column;
  box-shadow: 0 8px 32px rgba(20, 36, 46, 0.18);
  overflow: hidden;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 18px;
  border-bottom: 1px solid #eee;
  flex-shrink: 0;
}

.header-title {
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  color: #31414f;
}

.header-btn {
  border: 1px solid #d7dee6;
  background: #fff;
  color: #31414f;
  border-radius: 999px;
  padding: 8px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.header-btn:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.header-btn.primary {
  border-color: #176b3a;
  background: #176b3a;
  color: #fff;
}

.header-btn.primary:hover:not(:disabled) {
  background: #145a31;
  border-color: #145a31;
}

.source-caption {
  margin: 0;
  padding: 10px 18px 0;
  color: #5b6670;
  font-size: 12px;
  line-height: 1.5;
}

.source-caption code {
  background: #f2f4f6;
  border-radius: 4px;
  padding: 1px 4px;
  font-family: "SFMono-Regular", "Menlo", "Consolas", monospace;
  font-size: 11px;
}

.modal-error {
  margin: 10px 18px 0;
  padding: 8px 12px;
  border: 1px solid #efc6c6;
  border-radius: 10px;
  background: rgba(255, 245, 245, 0.96);
  color: #9f2f2f;
  font-size: 12px;
  font-weight: 600;
}

.lyric-textarea {
  margin: 12px 18px 18px;
  width: calc(100% - 36px);
  min-height: 320px;
  resize: vertical;
  border: 1px solid #cfd8df;
  border-radius: 10px;
  padding: 12px 14px;
  font-family: "SFMono-Regular", "Menlo", "Consolas", monospace;
  font-size: 13px;
  line-height: 1.55;
  color: #1b1f24;
  background: #fafafa;
  box-sizing: border-box;
  outline: none;
  transition: border-color 0.2s;
  overflow-y: auto;
}

.lyric-textarea:focus {
  border-color: #176b3a;
  background: #fff;
}

.lyric-textarea:disabled {
  cursor: not-allowed;
  opacity: 0.7;
}

@media (max-width: 720px) {
  .modal-overlay {
    padding: 0;
  }

  .modal-content {
    border-radius: 0;
    max-width: 100%;
    max-height: 100vh;
    height: 100vh;
  }

  .lyric-textarea {
    flex: 1;
    min-height: 0;
    margin: 12px 0 0;
    width: 100%;
    border-radius: 0;
    border-left: none;
    border-right: none;
    border-bottom: none;
  }
}
</style>
