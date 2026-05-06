export const FEATURE_GUIDE = `## Grimoire Feature Guide

### Keyboard Shortcuts
- Ctrl+P: Open Quick Switcher (fuzzy-search notes by title)
- Ctrl+F: Open Search panel (full-text + semantic)
- Ctrl+N: Create new note
- Ctrl+T: New blank tab
- Ctrl+Tab / Ctrl+Shift+Tab: Cycle tabs forward/backward
- Ctrl+W: Close active tab
- Ctrl+S: Save current note
- F11: Toggle focus / distraction-free mode (hides all panels)
- Ctrl+Shift+L: Lock vault
- Ctrl+Shift+Enter: Send selected text to chat
- Delete: Delete active note
- M (on kanban card): Start keyboard move mode
- Arrow keys: Navigate tabs, calendar, quick switcher, menus
- Enter: Send chat message / confirm dialog
- Escape: Cancel / close dialog / dismiss

### View Types
- Note editor: Markdown editing with read/edit toggle, #tags, [[wiki-links]]
- Graph: Force-directed graph of wiki-linked notes
- Kanban board: Notes grouped by a select property into columns
- Calendar: Daily notes grid + GitHub-style activity heatmap
- Database/table: Spreadsheet-style property editor with filters
- Search: Combined full-text + semantic search
- Chat: LLM chat sidebar or full-window tab

### Note Features
- #tags and [[wiki-links]] with automatic backlink tracking
- Note properties: text, number, date, boolean (checkbox), select (dropdown)
- Templates: Blank, Meeting Notes, Daily Journal, Book Notes + user-created
- Bookmarks: Pin frequently accessed notes above the folder tree
- Read/Edit mode toggle per note tab
- Daily notes with configurable date format
- Export current note or folder-list note: Markdown, HTML, or PDF (print dialog); bulk export from Settings

### Chat Features
- "Use notes" toggle: auto-injects relevant notes as context via RAG
- "Use view" toggle: auto-injects current board/table state as context
- Right-click messages: Copy, Copy as quote, Insert into note, Regenerate, Delete
- Send selection to chat (Ctrl+Shift+Enter or right-click → Send to Chat)
- Keep model in memory: prevents Ollama from unloading the model

### Settings
- LLM: Model selection, keep in memory toggle, embedding model
- Appearance: Theme (System/Light/Dark/Spellbook), accent colour, date format
- Security: Vault password (AES-GCM + Argon2id), per-folder passwords
- Data: Export all notes as Markdown; export single note from editor or folder list (Markdown, HTML, PDF via print)

### Vault Security
- Vault-level password encrypts all notes at rest
- Per-folder passwords for selective encryption
- Lock/unlock from Security settings or Ctrl+Shift+L
- Locked notes are hidden and unreadable until unlocked`;
