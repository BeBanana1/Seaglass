// Search mode of a track list on Android (#151): while the filter of a playlist or of the
// liked tracks has the focus or some text, the page hides its header and the sort to leave
// room for the results (the keyboard takes half the screen).
// Written by `TrackTools`, read by the views and by Back (`App`). Never active on desktop.

export const listSearch = $state({ active: false });

let exit: (() => void) | null = null;

/** Called by `TrackTools`; `onExit` clears the field and takes the focus away from it. */
export function setListSearch(active: boolean, onExit: (() => void) | null) {
  listSearch.active = active;
  exit = active ? onExit : null;
}

/** Leaves search mode (Android's Back). */
export function exitListSearch() {
  exit?.();
}
