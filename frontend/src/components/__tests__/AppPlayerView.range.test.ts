// Related documentation: `docs/range-playback.md`
import { afterEach, describe, expect, it, vi } from "vitest";
import { createApp, h, nextTick, reactive } from "vue";
import AppPlayerView from "../AppPlayerView.vue";

const mounted: Array<() => void> = [];

function mountPlayer(hasLyrics = true) {
  const props = reactive({
    isPlayerPanelVisible: true,
    isWideLayout: false,
    isLyricPanelVisible: true,
    selectMode: true,
    isLyricsLoading: false,
    hasLyrics,
    lyrics: hasLyrics
      ? [
          { time: 2, texts: ["First"] },
          { time: 5, texts: ["Second"] },
          { time: 9, texts: ["Third"] },
        ]
      : [],
    rawLyricsContent: hasLyrics ? "[00:02.00]First" : null,
    currentSongId: 6,
    currentSongKey: "remote:6",
    lyricApiBase: "/api",
    currentTime: 3,
    currentLyricIndex: 0,
    duration: 20,
    isPlaying: true,
    playMode: "sequential" as const,
  });
  const seeks: number[] = [];
  const container = document.createElement("div");
  document.body.appendChild(container);
  const app = createApp({
    render: () =>
      h(AppPlayerView, {
        ...props,
        onSeek: (time: number) => seeks.push(time),
      }),
  });
  app.mount(container);
  mounted.push(() => {
    app.unmount();
    container.remove();
  });
  const click = async (label: string) => {
    const button = Array.from(container.querySelectorAll("button")).find(
      (item) => item.textContent?.trim() === label,
    );
    expect(button, `button ${label}`).toBeDefined();
    button!.click();
    await nextTick();
  };
  return { props, seeks, container, click };
}

afterEach(() => {
  mounted.splice(0).forEach((unmount) => unmount());
  vi.useRealTimers();
});

describe("range playback", () => {
  it("loops from the first selected lyric through the end of the last selected lyric", async () => {
    const view = mountPlayer();
    await view.click("范围播放");
    await view.click("歌词范围");
    await view.click("开始循环");
    expect(view.seeks).toEqual([2]);

    view.props.currentTime = 9;
    await nextTick();
    expect(view.seeks).toEqual([2, 2]);
    await view.click("停止循环");
    view.props.currentTime = 10;
    await nextTick();
    expect(view.seeks).toEqual([2, 2]);
  });

  it("allows a time range without lyrics and clears it on song change", async () => {
    const view = mountPlayer(false);
    await view.click("范围播放");
    await view.click("时间范围");
    await view.click("开始循环");
    expect(view.seeks).toEqual([3]);

    view.props.currentSongKey = "remote:7";
    await nextTick();
    view.props.currentTime = 14;
    await nextTick();
    expect(view.seeks).toEqual([3]);
  });

  it("uses selected lyric boundaries and rejects a reversed time range", async () => {
    const view = mountPlayer();
    await view.click("范围播放");
    await view.click("歌词范围");
    const startSelect = view.container.querySelector<HTMLSelectElement>(
      'select[aria-label="起始歌词"]',
    )!;
    const endSelect = view.container.querySelector<HTMLSelectElement>(
      'select[aria-label="结束歌词"]',
    )!;
    startSelect.value = "1";
    startSelect.dispatchEvent(new Event("change"));
    endSelect.value = "2";
    endSelect.dispatchEvent(new Event("change"));
    await nextTick();
    await view.click("开始循环");
    expect(view.seeks).toEqual([5]);

    await view.click("范围播放");
    await view.click("时间范围");
    const startInput = view.container.querySelector<HTMLInputElement>(
      'input[aria-label="起始时间"]',
    )!;
    const endInput = view.container.querySelector<HTMLInputElement>(
      'input[aria-label="结束时间"]',
    )!;
    startInput.value = "0:12";
    startInput.dispatchEvent(new Event("input"));
    endInput.value = "0:10";
    endInput.dispatchEvent(new Event("input"));
    await nextTick();
    expect(
      Array.from(view.container.querySelectorAll("button")).find(
        (button) => button.textContent?.trim() === "更新范围",
      )?.disabled,
    ).toBe(true);
  });

  it("keeps the lyric scroll position fixed during range mode and resumes afterward", async () => {
    const view = mountPlayer();
    const lyricBody = view.container.querySelector<HTMLElement>(".lyric-body")!;
    const scrollTo = vi.fn();
    lyricBody.scrollTo = scrollTo;

    await view.click("范围播放");
    await view.click("歌词范围");
    view.props.currentLyricIndex = 1;
    await nextTick();
    expect(scrollTo).not.toHaveBeenCalled();

    await view.click("开始循环");
    view.props.currentLyricIndex = 2;
    await nextTick();
    expect(scrollTo).not.toHaveBeenCalled();

    await view.click("停止循环");
    view.props.currentLyricIndex = 0;
    await nextTick();
    expect(scrollTo).toHaveBeenCalledOnce();
  });
});
