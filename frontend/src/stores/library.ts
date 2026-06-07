import { defineStore } from "pinia";
import { ref } from "vue";
import { useLibrarySources } from "@/composables/useLibrarySources";
import { getRuntimeCapabilities } from "@/utils/platform";

export const useLibraryStore = defineStore("library", () => {
  const supportsRawContentPlayback = ref(false);
  const library = useLibrarySources({
    supportsRawContentPlayback,
  });

  const initRuntimeCapabilities = async () => {
    const runtimeCapabilities = await getRuntimeCapabilities();
    supportsRawContentPlayback.value =
      runtimeCapabilities.supportsRawContentPlayback;
  };

  return {
    supportsRawContentPlayback,
    initRuntimeCapabilities,
    ...library,
  };
});
