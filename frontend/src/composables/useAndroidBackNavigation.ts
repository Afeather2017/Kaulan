import { getRuntimeCapabilities } from "@/utils/platform";
import type { MainView, PlayerPanelMode } from "@/stores/ui";
import type { Ref } from "vue";

interface UseAndroidBackNavigationOptions {
  showFilterSheet: Ref<boolean>;
  selectedSourceMenuGroup: Ref<unknown | null>;
  selectedSongListMenuTitle: Ref<string | null>;
  selectedCollectionMenuName: Ref<string | null>;
  showActiveQueueModal: Ref<boolean>;
  showAddDeviceModal: Ref<boolean>;
  showUploadModal: Ref<boolean>;
  showOnlineSearchModal: Ref<boolean>;
  showLyricSearchModal: Ref<boolean>;
  showCreateCollection: Ref<boolean>;
  showAddToCollection: Ref<boolean>;
  showSettings: Ref<boolean>;
  selectMode: Ref<boolean>;
  clearSongSelection: () => void;
  collectionSelectMode: Ref<boolean>;
  selectedCollectionsList: Ref<Set<string>>;
  playerPanelMode: Ref<PlayerPanelMode>;
  isWideLayout: Ref<boolean>;
  currentView: Ref<MainView>;
  closeSourceMenu: () => void;
  closeSongListMenu: () => void;
  closeCollectionMenu: () => void;
  hideCreateCollectionModal: () => void;
  hideAddToCollectionModal: () => void;
  closeSettings: () => void;
  backToPlaylists: () => void;
}

export function useAndroidBackNavigation(
  options: UseAndroidBackNavigationOptions,
) {
  const {
    showFilterSheet,
    selectedSourceMenuGroup,
    selectedSongListMenuTitle,
    selectedCollectionMenuName,
    showActiveQueueModal,
    showAddDeviceModal,
    showUploadModal,
    showOnlineSearchModal,
    showLyricSearchModal,
    showCreateCollection,
    showAddToCollection,
    showSettings,
    selectMode,
    clearSongSelection,
    collectionSelectMode,
    selectedCollectionsList,
    playerPanelMode,
    isWideLayout,
    currentView,
    closeSourceMenu,
    closeSongListMenu,
    closeCollectionMenu,
    hideCreateCollectionModal,
    hideAddToCollectionModal,
    closeSettings,
    backToPlaylists,
  } = options;

  let androidBackListener: { unregister(): Promise<void> } | null = null;

  const closeTopOverlay = () => {
    if (showFilterSheet.value) {
      showFilterSheet.value = false;
      return true;
    }

    if (selectedSourceMenuGroup.value) {
      closeSourceMenu();
      return true;
    }

    if (selectedSongListMenuTitle.value) {
      closeSongListMenu();
      return true;
    }

    if (selectedCollectionMenuName.value) {
      closeCollectionMenu();
      return true;
    }

    if (showActiveQueueModal.value) {
      showActiveQueueModal.value = false;
      return true;
    }

    if (showAddDeviceModal.value) {
      showAddDeviceModal.value = false;
      return true;
    }

    if (showUploadModal.value) {
      showUploadModal.value = false;
      return true;
    }

    if (showOnlineSearchModal.value) {
      showOnlineSearchModal.value = false;
      return true;
    }

    if (showLyricSearchModal.value) {
      showLyricSearchModal.value = false;
      return true;
    }

    if (showCreateCollection.value) {
      hideCreateCollectionModal();
      return true;
    }

    if (showAddToCollection.value) {
      hideAddToCollectionModal();
      return true;
    }

    if (showSettings.value) {
      closeSettings();
      return true;
    }

    return false;
  };

  const handleAndroidBackPress = () => {
    if (closeTopOverlay()) {
      return true;
    }

    if (selectMode.value) {
      clearSongSelection();
      return true;
    }

    if (collectionSelectMode.value) {
      collectionSelectMode.value = false;
      selectedCollectionsList.value.clear();
      return true;
    }

    if (playerPanelMode.value !== "collapsed" && !isWideLayout.value) {
      playerPanelMode.value = "collapsed";
      return true;
    }

    if (currentView.value !== "playlists") {
      backToPlaylists();
      return true;
    }

    return false;
  };

  const handleActionBack = () => {
    if (playerPanelMode.value !== "collapsed" && !isWideLayout.value) {
      playerPanelMode.value = "collapsed";
      return;
    }
    backToPlaylists();
  };

  const registerAndroidBackHandler = async () => {
    const runtimeCapabilities = await getRuntimeCapabilities();
    if (!runtimeCapabilities.supportsAndroidBackHandler) {
      return;
    }

    try {
      const [{ onBackButtonPress }, { getCurrentWindow }] = await Promise.all([
        import("@tauri-apps/api/app"),
        import("@tauri-apps/api/window"),
      ]);

      androidBackListener = await onBackButtonPress(async ({ canGoBack }) => {
        if (handleAndroidBackPress()) {
          return;
        }

        if (canGoBack) {
          window.history.back();
          return;
        }

        await getCurrentWindow().close();
      });
    } catch (error) {
      console.warn("[app] Failed to register Android back handler:", error);
    }
  };

  const cleanupAndroidBackHandler = () => {
    if (androidBackListener) {
      void androidBackListener.unregister();
      androidBackListener = null;
    }
  };

  return {
    closeTopOverlay,
    handleAndroidBackPress,
    handleActionBack,
    registerAndroidBackHandler,
    cleanupAndroidBackHandler,
  };
}
