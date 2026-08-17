import { defineStore } from "pinia";
import { ref } from "vue";
import { useLibrarySources } from "@/composables/useLibrarySources";
import { getRuntimeCapabilities } from "@/utils/platform";

export const useLibraryStore = defineStore("library", () => {
  const supportsRawContentPlayback = ref(false);
  const library = useLibrarySources({
    supportsRawContentPlayback,
  });

  // Gate for the first source-group load of this session. Playback entry
  // points await it so a tap racing the initial `/playlists` fetch cannot
  // build a queue from pre-load fallback entries (basename paths → loopback
  // HTTP) when raw content:// playback is possible — the tap waits for the
  // library, then re-adopts live entries. Refreshes after the first load are
  // not gated; only the initial settle matters for the race.
  const initialSourceGroupsLoaded = ref<Promise<void> | null>(null);

  const initRuntimeCapabilities = async () => {
    const runtimeCapabilities = await getRuntimeCapabilities();
    supportsRawContentPlayback.value =
      runtimeCapabilities.supportsRawContentPlayback;
  };

  const waitForInitialSourceGroups = (): Promise<void> => {
    return initialSourceGroupsLoaded.value ?? Promise.resolve();
  };

  const refreshSourceGroups = async () => {
    const load = library.refreshSourceGroups();
    if (initialSourceGroupsLoaded.value === null) {
      initialSourceGroupsLoaded.value = load;
      // The promise stored on the ref settles even if the caller drops it.
      load.catch(() => {});
    }
    return load;
  };

  return {
    ...library,
    supportsRawContentPlayback,
    initRuntimeCapabilities,
    waitForInitialSourceGroups,
    // Overrides the spread composable entry so every caller funnels through
    // the gate-recording wrapper above.
    refreshSourceGroups,
  };
});
