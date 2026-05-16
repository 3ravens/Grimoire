// Panel layout store — sidebar open/close state, widths, and drag-to-resize logic.

const COLLAPSE_THRESHOLD = 80;

export function createPanelLayout() {
  let foldersOpen = $state(true);
  let notesOpen   = $state(true);
  let chatOpen    = $state(false);
  let focusMode   = $state(false);
  /** @type {{ foldersOpen: boolean, notesOpen: boolean } | null} */
  let savedLayout = null;

  let foldersWidth = $state(Number(localStorage.getItem('grimoire:foldersWidth')) || 200);
  let notesWidth   = $state(Number(localStorage.getItem('grimoire:notesWidth'))   || 240);
  let chatWidth    = $state(Number(localStorage.getItem('grimoire:chatWidth'))    || 360);

  // activeDrag: { panel: string, startX: number, startWidth: number } | null
  /** @type {{ panel: string, startX: number, startWidth: number } | null} */
  let activeDrag = $state(null);

  const gridCols = $derived.by(() => {
    if (focusMode) {
      return [
        '1fr',
        ...(chatOpen ? ['5px', `${chatWidth}px`] : []),
      ].join(' ');
    }
    return [
      foldersOpen ? `${foldersWidth}px` : '28px',
      foldersOpen ? '5px' : '0px',
      notesOpen   ? `${notesWidth}px`   : '28px',
      notesOpen   ? '5px' : '0px',
      '1fr',
      ...(chatOpen ? ['5px', `${chatWidth}px`] : []),
    ].join(' ');
  });

  $effect(() => {
    document.body.classList.toggle('focus-mode', focusMode);
  });

  /** @param {string} panel @param {number} width */
  function savePanelWidth(panel, width) {
    localStorage.setItem(`grimoire:${panel}Width`, String(width));
  }

  /** @param {string} panel @param {MouseEvent} e */
  function startDrag(panel, e) {
    e.preventDefault();
    const startWidth = panel === 'folders' ? foldersWidth
                     : panel === 'notes'   ? notesWidth
                     : chatWidth;
    activeDrag = { panel, startX: e.clientX, startWidth };
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  /** @param {MouseEvent} e */
  function onDragMove(e) {
    if (!activeDrag) return;
    const delta = e.clientX - activeDrag.startX;
    const newWidth = activeDrag.panel === 'chat'
      ? activeDrag.startWidth - delta
      : activeDrag.startWidth + delta;

    if (newWidth < COLLAPSE_THRESHOLD) {
      savePanelWidth(activeDrag.panel, activeDrag.startWidth);
      if (activeDrag.panel === 'folders') foldersOpen = false;
      else if (activeDrag.panel === 'notes') notesOpen = false;
      else chatOpen = false;
      activeDrag = null;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      return;
    }

    const clamped = Math.max(COLLAPSE_THRESHOLD, newWidth);
    if (activeDrag.panel === 'folders') foldersWidth = clamped;
    else if (activeDrag.panel === 'notes') notesWidth = clamped;
    else chatWidth = clamped;
  }

  function onDragEnd() {
    if (!activeDrag) return;
    const width = activeDrag.panel === 'folders' ? foldersWidth
                : activeDrag.panel === 'notes'   ? notesWidth
                : chatWidth;
    savePanelWidth(activeDrag.panel, width);
    activeDrag = null;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }

  function toggleFocusMode() {
    if (focusMode) {
      if (savedLayout) {
        foldersOpen = savedLayout.foldersOpen;
        notesOpen   = savedLayout.notesOpen;
        savedLayout = null;
      }
      focusMode = false;
    } else {
      savedLayout = { foldersOpen, notesOpen };
      foldersOpen = false;
      notesOpen   = false;
      focusMode   = true;
    }
  }

  return {
    get foldersOpen() { return foldersOpen; },
    set foldersOpen(v) { foldersOpen = v; },
    get notesOpen() { return notesOpen; },
    set notesOpen(v) { notesOpen = v; },
    get chatOpen() { return chatOpen; },
    set chatOpen(v) { chatOpen = v; },
    get focusMode() { return focusMode; },
    get foldersWidth() { return foldersWidth; },
    get notesWidth() { return notesWidth; },
    get chatWidth() { return chatWidth; },
    get activeDrag() { return activeDrag; },
    get gridCols() { return gridCols; },
    startDrag,
    onDragMove,
    onDragEnd,
    toggleFocusMode,
  };
}
