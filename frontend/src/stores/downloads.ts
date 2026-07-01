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
}

interface CreateDownloadJobResponse {
  success: boolean;
  message: string;
  job_id?: string | null;
}

type JobWaiter = {
  resolve: (snapshot: DownloadJobSnapshot) => void;
  reject: (error: Error) => void;
};

const POLL_INTERVAL_MS = 1000;

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
              state: "completed",
              phase: "completed",
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
          };

          if (snapshot.state === "completed" || snapshot.state === "failed") {
            finalizeJob(job.key, snapshot);
          }
        } catch (error) {
          finalizeJob(job.key, {
            ...job.snapshot,
            state: "failed",
            phase: "failed",
            error: String(error),
            message: String(error),
          });
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
    if (!response.ok || !payload.success || !payload.job_id) {
      throw new Error(payload.message || "下载任务创建失败");
    }

    const snapshot: DownloadJobSnapshot = {
      job_id: payload.job_id,
      source: String(request.source ?? "unknown"),
      state: "queued",
      phase: "queued",
      percent: null,
      message: payload.message || `Queued download: ${title}`,
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
    };
    ensurePolling();
    void pollJobs();

    return {
      key,
      jobId: payload.job_id,
    };
  };

  return {
    jobs,
    activeJobs,
    hasActiveJobs,
    ensurePolling,
    pollJobs,
    startDownloadJob,
    waitForJob,
  };
});
