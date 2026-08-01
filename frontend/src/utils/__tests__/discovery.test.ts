import { afterEach, describe, expect, it, vi } from "vitest";

import {
  fetchDeviceResolution,
  markDeviceResolved,
  refreshDiscoveredDevices,
} from "@/utils/discovery";

vi.mock("@/utils/api", () => ({
  getLocalApiBase: () => "http://localhost:2080/api",
  normalizeApiBase: (value: string) => value,
}));

describe("discovery scan timing", () => {
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("observes for 20 seconds while sending only three UDP requests", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-31T00:00:00.000Z"));
    const startedAt = Date.now();
    let finishedAt = 0;

    global.fetch = vi.fn(async (input) => {
      const url = String(input);
      if (url.endsWith("/discovery/scan/finish")) {
        finishedAt = Date.now();
      }
      return {
        ok: true,
        statusText: "OK",
        json: async () =>
          url.endsWith("/discovery/devices")
            ? []
            : { success: true, message: "ok" },
      } as Response;
    }) as typeof fetch;

    const scan = refreshDiscoveredDevices({ windowMs: 20_000 });
    await vi.advanceTimersByTimeAsync(20_000);
    await scan;

    const urls = vi
      .mocked(global.fetch)
      .mock.calls.map(([input]) => String(input));
    expect(
      urls.filter((url) => url.endsWith("/discovery/request")),
    ).toHaveLength(3);
    expect(finishedAt - startedAt).toBeGreaterThanOrEqual(20_000);
  });

  it("stops a recovery scan as soon as its target is visible", async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-07-31T00:00:00.000Z"));
    let devicePolls = 0;
    let finishedAt = 0;
    const startedAt = Date.now();

    global.fetch = vi.fn(async (input) => {
      const url = String(input);
      if (url.endsWith("/discovery/devices")) {
        devicePolls += 1;
      }
      if (url.endsWith("/discovery/scan/finish")) {
        finishedAt = Date.now();
      }
      return {
        ok: true,
        statusText: "OK",
        json: async () => {
          if (url.endsWith("/discovery/devices")) {
            return devicePolls >= 2
              ? [
                  {
                    device_id: "wanted-device",
                    device_name: "Wanted",
                    api_url: "http://192.168.1.20:2080/api",
                    last_seen_secs_ago: 0,
                  },
                ]
              : [];
          }
          return { success: true, message: "ok" };
        },
      } as Response;
    }) as typeof fetch;

    const scan = refreshDiscoveredDevices({
      windowMs: 20_000,
      shouldStop: (devices) =>
        devices.some((device) => device.device_id === "wanted-device"),
    });
    await vi.advanceTimersByTimeAsync(1000);
    await scan;

    expect(finishedAt - startedAt).toBe(1000);
  });
});

describe("playback device resolutions", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("publishes verified addresses to localhost", async () => {
    global.fetch = vi.fn(
      async () => ({ ok: true, status: 200 }) as Response,
    ) as typeof fetch;

    await markDeviceResolved("remote-device", "http://192.168.1.20:2080/api");

    expect(global.fetch).toHaveBeenCalledWith(
      "http://localhost:2080/api/discovery/resolutions/remote-device",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({
          api_url: "http://192.168.1.20:2080/api",
        }),
      }),
    );
  });

  it("treats an absent session mark as unavailable", async () => {
    global.fetch = vi.fn(
      async () => ({ ok: false, status: 404 }) as Response,
    ) as typeof fetch;

    await expect(fetchDeviceResolution("missing-device")).resolves.toBeNull();
  });
});
