import { onMounted, onBeforeUnmount, ref, watch } from "vue";

import { getAllowTextSelection, setAllowTextSelection } from "@/utils/storage";

// Related documentation: `docs/text-selection.md`

const STYLE_ID = "kaulan-text-selection-style";

const allowed = ref(getAllowTextSelection());

let styleEl: HTMLStyleElement | null = null;
let consumerCount = 0;

function buildCss(allowSelection: boolean): string {
  if (allowSelection) {
    return "";
  }

  // Issue #30: the UI should look like an app, not a web page. Block
  // mouse selection everywhere except form fields where users still need
  // to highlight/edit text. Images need their own treatment: browsers let
  // users drag <img> elements even when the parent has user-select:none,
  // so we also disable -webkit-user-drag and force user-select:none on img.
  return `
    body, html {
      -webkit-user-select: none;
      -moz-user-select: none;
      -ms-user-select: none;
      user-select: none;
    }
    img {
      -webkit-user-drag: none;
      -webkit-user-select: none;
      -moz-user-select: none;
      -ms-user-select: none;
      user-select: none;
    }
    input, textarea, select,
    [contenteditable="true"],
    [contenteditable=""] {
      -webkit-user-select: text;
      -moz-user-select: text;
      -ms-user-select: text;
      user-select: text;
    }
  `;
}

function ensureStyleElement(): HTMLStyleElement {
  if (styleEl && document.head.contains(styleEl)) {
    return styleEl;
  }

  const el = document.createElement("style");
  el.id = STYLE_ID;
  document.head.appendChild(el);
  styleEl = el;
  return el;
}

function syncStyle(): void {
  const el = ensureStyleElement();
  el.textContent = buildCss(allowed.value);
}

/**
 * Apply the global text-selection preference and keep it in sync with
 * localStorage. Mount this once from the root component. Multiple consumers
 * are supported via ref-counting; the style element is removed only after the
 * last consumer unmounts.
 */
export function useTextSelection() {
  onMounted(() => {
    consumerCount += 1;
    syncStyle();
  });

  onBeforeUnmount(() => {
    consumerCount -= 1;
    if (consumerCount <= 0 && styleEl) {
      styleEl.remove();
      styleEl = null;
      consumerCount = 0;
    }
  });

  watch(allowed, syncStyle);

  const setAllowTextSelectionState = (value: boolean) => {
    allowed.value = value;
    setAllowTextSelection(value);
  };

  return {
    allowTextSelection: allowed,
    setAllowTextSelectionState,
  };
}
