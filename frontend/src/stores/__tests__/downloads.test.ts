import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

import { useDownloadsStore } from "@/stores/downloads";

const LOST_JOB_MESSAGE = "任务已丢失（服务可能已重启）";
const POLL_RETRY_MESSAGE = "连接下载服务失败，正在重试";

const createJsonResponse = (body: unknown, status = 200) => ({
  ok: status >= 200 && status < 300,
  status,
  json: async () => body,
});

describe("downloads store polling", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.unstubAllGlobals();
    vi.stubGlobal("window", {
      clearInterval: vi.fn(),
      setInterval: vi.fn(() => 1),
    });
  });

  it("treats a missing backend job as failed instead of completed", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        createJsonResponse({
          success: true,
          message: "下载任务已创建",
          job_id: "job-1",
        }),
      )
      .mockResolvedValueOnce(
        createJsonResponse({
          job_id: "job-1",
          source: "youtube",
          state: "running",
          phase: "downloading",
          percent: 10,
          message: "Downloading",
          detail: null,
          filename: null,
          warning: null,
          error: null,
        }),
      )
      .mockResolvedValueOnce({
        ok: false,
        status: 404,
        json: async () => null,
      });
    vi.stubGlobal("fetch", fetchMock);

    const store = useDownloadsStore();
    const { key } = await store.startDownloadJob(
      "http://localhost:2080/api",
      "Song",
      "youtube:1",
      { source: "youtube" },
    );

    const waiter = store.waitForJob(key);
    await store.pollJobs();

    await expect(waiter).rejects.toThrow(LOST_JOB_MESSAGE);
    expect(store.activeJobs).toHaveLength(1);
    expect(store.activeJobs[0]?.snapshot.state).toBe("failed");
    store.dismissJob(key);
    expect(store.activeJobs).toHaveLength(0);
  });

  it("keeps polling through transient failures before giving up", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        createJsonResponse({
          success: true,
          message: "下载任务已创建",
          job_id: "job-2",
        }),
      )
      .mockResolvedValueOnce(
        createJsonResponse({
          job_id: "job-2",
          source: "youtube",
          state: "running",
          phase: "downloading",
          percent: 10,
          message: "Downloading",
          detail: null,
          filename: null,
          warning: null,
          error: null,
        }),
      )
      .mockRejectedValueOnce(new Error("network down"))
      .mockRejectedValueOnce(new Error("network down"))
      .mockRejectedValueOnce(new Error("network down"));
    vi.stubGlobal("fetch", fetchMock);

    const store = useDownloadsStore();
    const { key } = await store.startDownloadJob(
      "http://localhost:2080/api",
      "Song",
      "youtube:2",
      { source: "youtube" },
    );

    await store.pollJobs();
    expect(store.activeJobs).toHaveLength(1);
    expect(store.activeJobs[0]?.snapshot.message).toBe(POLL_RETRY_MESSAGE);

    await store.pollJobs();
    expect(store.activeJobs).toHaveLength(1);
    expect(store.activeJobs[0]?.snapshot.message).toBe(POLL_RETRY_MESSAGE);

    const waiter = store.waitForJob(key);
    await store.pollJobs();

    await expect(waiter).rejects.toThrow("连接下载服务失败");
    expect(store.activeJobs).toHaveLength(1);
    expect(store.activeJobs[0]?.snapshot.state).toBe("failed");
    store.dismissJob(key);
    expect(store.activeJobs).toHaveLength(0);
  });

  it("keeps completed jobs visible until dismissed", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce(
        createJsonResponse({
          success: true,
          message: "下载任务已创建",
          job_id: "job-3",
        }),
      )
      .mockResolvedValueOnce(
        createJsonResponse({
          job_id: "job-3",
          source: "youtube",
          state: "running",
          phase: "downloading",
          percent: 10,
          message: "Downloading",
          detail: null,
          filename: null,
          warning: null,
          error: null,
        }),
      )
      .mockResolvedValueOnce(
        createJsonResponse({
          job_id: "job-3",
          source: "youtube",
          state: "completed",
          phase: "completed",
          percent: 100,
          message: "Done",
          detail: null,
          filename: "song.mp3",
          warning: null,
          error: null,
        }),
      );
    vi.stubGlobal("fetch", fetchMock);

    const store = useDownloadsStore();
    const { key } = await store.startDownloadJob(
      "http://localhost:2080/api",
      "Song",
      "youtube:3",
      { source: "youtube" },
    );

    const waiter = store.waitForJob(key);
    await store.pollJobs();

    await expect(waiter).resolves.toMatchObject({ state: "completed" });
    expect(store.activeJobs).toHaveLength(1);
    expect(store.activeJobs[0]?.snapshot.message).toBe("Done");

    store.dismissJob(key);
    expect(store.activeJobs).toHaveLength(0);
  });
});
