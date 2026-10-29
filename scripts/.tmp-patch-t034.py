from pathlib import Path

path = Path(r"D:\Dev\Projects\Grimoire\src\lib\NoteEditor.svelte")
text = path.read_text(encoding="utf-8")

old_import = """    import { applyEditorTab } from \"./utils/editorIndent.js\";
    import {
        applyChecklistToggle,
        applyListEnter,
        applyListTab,
        toggleChecklistAtIndex,
    } from \"./utils/editorLists.js\";"""

new_import = """    import { applyEditorTab } from \"./utils/editorIndent.js\";
    import { applyBlockquoteEnter } from \"./utils/editorBlockquotes.js\";
    import {
        applyChecklistToggle,
        applyListEnter,
        applyListTab,
        toggleChecklistAtIndex,
    } from \"./utils/editorLists.js\";"""

old_enter = """        // Enter: continue / exit / split Markdown list items.
        if (
            isEnterKey(e) &&
            !e.ctrlKey &&
            !e.metaKey &&
            !e.altKey &&
            !e.shiftKey
        ) {
            const listNext = applyListEnter(value, start, end);
            if (listNext) {
                e.preventDefault();
                applyEditorEdit(el, listNext);
            }
            return;
        }"""

new_enter = """        // Enter: continue / exit / split Markdown list or blockquote lines.
        if (
            isEnterKey(e) &&
            !e.ctrlKey &&
            !e.metaKey &&
            !e.altKey &&
            !e.shiftKey &&
            !e.isComposing
        ) {
            const listNext = applyListEnter(value, start, end);
            if (listNext) {
                e.preventDefault();
                applyEditorEdit(el, listNext);
                return;
            }
            const quoteNext = applyBlockquoteEnter(value, start, end);
            if (quoteNext) {
                e.preventDefault();
                applyEditorEdit(el, quoteNext);
            }
            return;
        }"""

if old_import not in text:
    raise SystemExit("import block not found")
if old_enter not in text:
    raise SystemExit("enter block not found")

path.write_text(
    text.replace(old_import, new_import, 1).replace(old_enter, new_enter, 1),
    encoding="utf-8",
)
print("patched ok")
