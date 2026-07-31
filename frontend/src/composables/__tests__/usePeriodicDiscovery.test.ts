/**
 * Tests for usePeriodicDiscovery composable
 *
 * @module composables/__tests__/usePeriodicDiscovery.test
 *
 * Related documentation:
 * - `docs/device-discovery.md`
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { usePeriodicDiscovery } from "../usePeriodicDiscovery";

vi.mock("@/utils/api", () => ({
  getLocalApiBase: () => "http://localhost:2080/api",
}));

const jsonResponse = (
  body: unknown,
  init: { ok?: boolean; status?: number } = {},
): Response => {
  const ok = init.ok ?? true;
  return {
    ok,
    status: init.status ?? (ok ? 200 : 500),
    json: async () => body,
  } as unknown as Response;
};

describe("usePeriodicDiscovery", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("load() sets enabled from GET /discovery/periodic", async () => {
    const fetchMock = vi
      .spyOn(global, "fetch")
      .mockResolvedValue(jsonResponse({ enabled: false }));
    const { enabled, hasError, message, load } = usePeriodicDiscovery();

    await load();

    expect(enabled.value).toBe(false);
    expect(hasError.value).toBe(false);
    expect(message.value).toBe("");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://localhost:2080/api/discovery/periodic");
    expect(init?.method ?? "GET").toBe("GET");
  });

  it("load() treats missing `enabled` as enabled (back-compat)", async () => {
    vi.spyOn(global, "fetch").mockResolvedValue(jsonResponse({}));
    const { enabled, load } = usePeriodicDiscovery();

    await load();

    expect(enabled.value).toBe(true);
  });

  it("load() surfaces an error when the request fails", async () => {
    vi.spyOn(global, "fetch").mockRejectedValue(new Error("network down"));
    const { enabled, hasError, message, load } = usePeriodicDiscovery();

    await load();

    expect(enabled.value).toBe(true); // unchanged default
    expect(hasError.value).toBe(true);
    expect(message.value).toBe("读取定期发现设置失败。");
  });

  it("save(true) sends PUT with the expected body and keeps the new value", async () => {
    const fetchMock = vi
      .spyOn(global, "fetch")
      .mockResolvedValue(jsonResponse({ success: true }));
    const { enabled, isSaving, message, save } = usePeriodicDiscovery();

    const ok = await save(true);

    expect(ok).toBe(true);
    expect(enabled.value).toBe(true);
    expect(isSaving.value).toBe(false);
    expect(message.value).toBe("定期发现已开启。");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe("http://localhost:2080/api/discovery/periodic");
    expect(init?.method).toBe("PUT");
    expect(init?.headers).toEqual({ "Content-Type": "application/json" });
    expect(init?.body).toBe(JSON.stringify({ enabled: true }));
  });

  it("save(false) reports the disabled message", async () => {
    vi.spyOn(global, "fetch").mockResolvedValue(
      jsonResponse({ success: true }),
    );
    const { enabled, message, save } = usePeriodicDiscovery();

    await save(false);

    expect(enabled.value).toBe(false);
    expect(message.value).toBe("定期发现已关闭，手动刷新仍然可用。");
  });

  it("save() rolls back enabled and flags an error when the server refuses", async () => {
    vi.spyOn(global, "fetch").mockResolvedValue(
      jsonResponse(
        { success: false, message: "disk full" },
        { ok: false, status: 500 },
      ),
    );
    const { enabled, hasError, message, save } = usePeriodicDiscovery();
    await save(true); // flip to true first so rollback is observable
    vi.spyOn(global, "fetch").mockResolvedValue(
      jsonResponse(
        { success: false, message: "disk full" },
        { ok: false, status: 500 },
      ),
    );

    const ok = await save(false);

    expect(ok).toBe(false);
    expect(enabled.value).toBe(true); // rolled back to prior value
    expect(hasError.value).toBe(true);
    expect(message.value).toContain("保存定期发现设置失败");
    expect(message.value).toContain("disk full");
  });

  it("save() rolls back when the network throws", async () => {
    const { enabled, hasError, save } = usePeriodicDiscovery();

    const ok = await save(false);

    expect(ok).toBe(false);
    expect(enabled.value).toBe(true); // default; never flipped to false
    expect(hasError.value).toBe(true);
  });
});
