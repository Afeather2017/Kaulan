import { describe, expect, it, vi } from "vitest";

import { downloadBlob, triggerAnchorDownload } from "@/utils/browserDownload";

// Related documentation: `docs/library-import.md`

describe("browserDownload", () => {
  it("triggerAnchorDownload clicks a hidden anchor with the url and no download attribute", () => {
    const click = vi.fn();
    const anchor = {
      href: "",
      download: "",
      rel: "",
      style: { display: "" },
      click,
    } as unknown as HTMLAnchorElement;

    const createElement = vi
      .spyOn(document, "createElement")
      .mockReturnValue(anchor);
    const appendChild = vi
      .spyOn(document.body, "appendChild")
      .mockImplementation((node) => node);
    const removeChild = vi
      .spyOn(document.body, "removeChild")
      .mockImplementation((node) => node);

    const url = "http://remote:2080/api/music/id/3?download=1";
    triggerAnchorDownload(url);

    expect(createElement).toHaveBeenCalledWith("a");
    expect(anchor.href).toBe(url);
    // Cross-origin: the server supplies the filename via Content-Disposition,
    // so the download attribute must NOT be set.
    expect(anchor.download).toBe("");
    expect(click).toHaveBeenCalledOnce();
    expect(appendChild).toHaveBeenCalledWith(anchor);
    expect(removeChild).toHaveBeenCalledWith(anchor);

    createElement.mockRestore();
    appendChild.mockRestore();
    removeChild.mockRestore();
  });

  it("downloadBlob triggers a download for a small payload and revokes the url", () => {
    vi.useFakeTimers();
    const revokeObjectURL = vi.fn();
    const urlCreator = vi
      .spyOn(URL, "createObjectURL")
      .mockReturnValue("blob:fake-url");
    const revokeSpy = vi
      .spyOn(URL, "revokeObjectURL")
      .mockImplementation(revokeObjectURL);

    const click = vi.fn();
    const anchor = {
      href: "",
      download: "",
      rel: "",
      style: { display: "" },
      click,
    } as unknown as HTMLAnchorElement;
    const createElement = vi
      .spyOn(document, "createElement")
      .mockReturnValue(anchor);
    vi.spyOn(document.body, "appendChild").mockImplementation((node) => node);
    vi.spyOn(document.body, "removeChild").mockImplementation((node) => node);

    downloadBlob(new Blob(["hi"]), "track.lrc");
    expect(click).toHaveBeenCalledOnce();
    expect(anchor.download).toBe("track.lrc");
    expect(revokeObjectURL).not.toHaveBeenCalled();

    vi.advanceTimersByTime(31_000);
    expect(revokeObjectURL).toHaveBeenCalledTimes(1);

    urlCreator.mockRestore();
    revokeSpy.mockRestore();
    createElement.mockRestore();
    vi.useRealTimers();
  });
});
