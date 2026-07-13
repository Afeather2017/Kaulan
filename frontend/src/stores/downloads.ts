import { defineStore } from "pinia";
import { computed, ref } from "vue";

export type DownloadPhase =
  | "queued"
  | "preparing"
  | "resolving_meta"
  | "downloading"
  | "post_processing"
  | "embedding_cover"
  | "saving_lyrics"
  | "refreshing_library"
  | "completed"
  | "failed";

export interface DownloadJobSnapshot {
  job_id: string;
  source: string;
  state: "queued" | "running" | "completed" | "failed";
  phase: DownloadPhase;
  percent: number | null;
  message: string;
  detail: string | null;
  filename: string | null;
  warning: string | null;
  error: string | null;
}

export interface ActiveDownloadJob {
  key: string;
  apiBase: string;
  title: string;
  resultKey: string;
  snapshot: DownloadJobSnapshot;
  pollFailures: number;
}

interface CreateDownloadJobResponse {
  success: boolean;
  message: string;
  job_id?: string | null;
}

/**
 * Request body for `POST /api/library/import-from-remote` (Tauri runtimes).
 * The local backend pulls each item's audio (and lyrics) from the remote server.
 * Related docs: `docs/library-import.md`.
 */
export interface ImportFromRemoteRequest {
  remote_api_base: string;
  items: Array<{ music_id: number; filename?: string }>;
  include_lyrics?: boolean;
}

type JobWaiter = {
  resolve: (snapshot: DownloadJobSnapshot) => void;
  reject: (error: Error) => void;
};

const POLL_INTERVAL_MS = 1000;
const POLL_FAILURE_LIMIT = 3;
const LOST_JOB_MESSAGE = "任务已丢失（服务可能已重启）";
const POLL_RETRY_MESSAGE = "连接下载服务失败，正在重试";

export const useDownloadsStore = defineStore("downloads", () => {
  const jobs = ref<Record<string, ActiveDownloadJob>>({});
  const pollTimer = ref<number | null>(null);
  const waiters = new Map<string, JobWaiter[]>();

  const activeJobs = computed(() =>
    Object.values(jobs.value).sort((left, right) =>
      left.title.localeCompare(right.title),
    ),
  );

  const hasActiveJobs = computed(() => activeJobs.value.length > 0);

  const buildJobKey = (apiBase: string, jobId: string) =>
    `${apiBase}::${jobId}`;

  const clearPolling = () => {
    if (pollTimer.value !== null && typeof window !== "undefined") {
      window.clearInterval(pollTimer.value);
      pollTimer.value = null;
    }
  };

  const finalizeJob = (jobKey: string, snapshot: DownloadJobSnapshot) => {
    const pendingWaiters = waiters.get(jobKey) ?? [];
    waiters.delete(jobKey);
    for (const waiter of pendingWaiters) {
      if (snapshot.state === "failed") {
        waiter.reject(new Error(snapshot.error || snapshot.message));
      } else {
        waiter.resolve(snapshot);
      }
    }
    delete jobs.value[jobKey];
    if (Object.keys(jobs.value).length === 0) {
      clearPolling();
    }
  };

  const pollJobs = async () => {
    const entries = Object.values(jobs.value);
    if (entries.length === 0) {
      clearPolling();
      return;
    }

    await Promise.all(
      entries.map(async (job) => {
        try {
          const response = await fetch(
            `${job.apiBase}/download/jobs/${job.snapshot.job_id}`,
          );
          if (response.status === 404) {
            finalizeJob(job.key, {
              ...job.snapshot,
              state: "failed",
              phase: "failed",
              error: LOST_JOB_MESSAGE,
              message: LOST_JOB_MESSAGE,
            });
            return;
          }
          if (!response.ok) {
            throw new Error(`Failed to poll download job: ${response.status}`);
          }

          const snapshot = (await response.json()) as DownloadJobSnapshot;
          jobs.value[job.key] = {
            ...job,
            snapshot,
            pollFailures: 0,
          };

          if (snapshot.state === "completed" || snapshot.state === "failed") {
            finalizeJob(job.key, snapshot);
          }
        } catch (error) {
          const nextFailures = job.pollFailures + 1;
          const errorMessage =
            nextFailures >= POLL_FAILURE_LIMIT
              ? `连接下载服务失败: ${String(error)}`
              : POLL_RETRY_MESSAGE;
          const nextSnapshot: DownloadJobSnapshot = {
            ...job.snapshot,
            detail: String(error),
            message: errorMessage,
            warning: job.snapshot.warning,
            error:
              nextFailures >= POLL_FAILURE_LIMIT
                ? errorMessage
                : job.snapshot.error,
            state:
              nextFailures >= POLL_FAILURE_LIMIT
                ? "failed"
                : job.snapshot.state,
            phase:
              nextFailures >= POLL_FAILURE_LIMIT
                ? "failed"
                : job.snapshot.phase,
          };

          jobs.value[job.key] = {
            ...job,
            snapshot: nextSnapshot,
            pollFailures: nextFailures,
          };

          if (nextFailures >= POLL_FAILURE_LIMIT) {
            finalizeJob(job.key, nextSnapshot);
          }
        }
      }),
    );
  };

  const ensurePolling = () => {
    if (pollTimer.value !== null || typeof window === "undefined") {
      return;
    }
    pollTimer.value = window.setInterval(() => {
      void pollJobs();
    }, POLL_INTERVAL_MS);
  };

  const waitForJob = (jobKey: string) =>
    new Promise<DownloadJobSnapshot>((resolve, reject) => {
      const snapshot = jobs.value[jobKey]?.snapshot;
      if (!snapshot) {
        reject(new Error("Download job not found"));
        return;
      }
      if (snapshot.state === "completed") {
        resolve(snapshot);
        return;
      }
      if (snapshot.state === "failed") {
        reject(new Error(snapshot.error || snapshot.message));
        return;
      }
      const pendingWaiters = waiters.get(jobKey) ?? [];
      pendingWaiters.push({ resolve, reject });
      waiters.set(jobKey, pendingWaiters);
    });

  /**
   * Register a freshly created job (online download or remote-library import)
   * and begin polling it. Shared by `startDownloadJob` and `startImportJob`.
   */
  const registerJob = (
    apiBase: string,
    title: string,
    resultKey: string,
    sourceLabel: string,
    payload: CreateDownloadJobResponse,
    queuedMessage: string,
  ): { key: string; jobId: string } => {
    if (!payload.success || !payload.job_id) {
      throw new Error(payload.message || "下载任务创建失败");
    }

    const snapshot: DownloadJobSnapshot = {
      job_id: payload.job_id,
      source: sourceLabel,
      state: "queued",
      phase: "queued",
      percent: null,
      message: queuedMessage,
      detail: null,
      filename: null,
      warning: null,
      error: null,
    };
    const key = buildJobKey(apiBase, payload.job_id);
    jobs.value[key] = {
      key,
      apiBase,
      title,
      resultKey,
      snapshot,
      pollFailures: 0,
    };
    ensurePolling();
    void pollJobs();

    return {
      key,
      jobId: payload.job_id,
    };
  };

  const startDownloadJob = async (
    apiBase: string,
    title: string,
    resultKey: string,
    request: Record<string, unknown>,
  ) => {
    const response = await fetch(`${apiBase}/download/jobs`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    const payload = (await response.json()) as CreateDownloadJobResponse;
    if (!response.ok) {
      throw new Error(payload.message || "下载任务创建失败");
    }
    return registerJob(
      apiBase,
      title,
      resultKey,
      String(request.source ?? "unknown"),
      payload,
      payload.message || `Queued download: ${title}`,
    );
  };

  /**
   * Start a remote-library import job on the LOCAL backend (Tauri runtimes
   * only). The local backend pulls the selected tracks from `remote_api_base`
   * into its `download_root`. Progress is polled through the same job store as
   * online downloads.
   */
  const startImportJob = async (
    apiBase: string,
    title: string,
    request: ImportFromRemoteRequest,
  ) => {
    const response = await fetch(`${apiBase}/library/import-from-remote`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(request),
    });
    const payload = (await response.json()) as CreateDownloadJobResponse;
    if (!response.ok) {
      throw new Error(payload.message || "导入任务创建失败");
    }
    return registerJob(
      apiBase,
      title,
      "import",
      "import",
      payload,
      payload.message || `Queued import: ${title}`,
    );
  };

  return {
    jobs,
    activeJobs,
    hasActiveJobs,
    ensurePolling,
    pollJobs,
    startDownloadJob,
    startImportJob,
    waitForJob,
  };
});
