import { renderTransclusionMarkdownToHtml } from './transclusion.js';

/** Mirrors `.read-mode-content` in editor.css for standalone HTML/PDF export. */
const READ_MODE_EXPORT_CSS = `
:root {
  color-scheme: light dark;
}
@media print {
  body { margin: 12mm; }
}
body {
  margin: 0;
  padding: 24px;
  font: 15px/1.7 ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  color: #1a1a1a;
  background: #faf9f6;
}
@media (prefers-color-scheme: dark) {
  body {
    color: #e8e4dc;
    background: #12100e;
  }
}
.read-mode-content h1,
.read-mode-content h2,
.read-mode-content h3,
.read-mode-content h4 {
  font-family: system-ui, Segoe UI, Roboto, Helvetica, Arial, sans-serif;
  margin: 1.2em 0 0.4em;
}
.read-mode-content p {
  margin: 0 0 0.8em;
}
.read-mode-content ul,
.read-mode-content ol {
  padding-left: 1.6em;
  margin: 0 0 0.8em;
}
.read-mode-content code {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.9em;
  background: rgba(0, 0, 0, 0.06);
  padding: 1px 4px;
  border-radius: 3px;
}
@media (prefers-color-scheme: dark) {
  .read-mode-content code {
    background: rgba(255, 255, 255, 0.08);
  }
}
.read-mode-content pre {
  background: rgba(0, 0, 0, 0.06);
  border: 1px solid rgba(0, 0, 0, 0.12);
  border-radius: 4px;
  padding: 12px 16px;
  overflow-x: auto;
  margin: 0 0 0.8em;
}
@media (prefers-color-scheme: dark) {
  .read-mode-content pre {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.12);
  }
}
.read-mode-content pre code {
  background: none;
  padding: 0;
}
.read-mode-content blockquote {
  border-left: 3px solid #a32121;
  margin: 0 0 0.8em;
  padding: 4px 0 4px 14px;
  opacity: 0.92;
}
.read-mode-content hr {
  border: none;
  border-top: 1px solid rgba(0, 0, 0, 0.15);
  margin: 1.2em 0;
}
@media (prefers-color-scheme: dark) {
  .read-mode-content hr {
    border-top-color: rgba(255, 255, 255, 0.15);
  }
}
.read-mode-content a {
  color: #a32121;
  text-decoration: underline;
}
.note-embed {
  margin: 0.75em 0;
}
.note-embed-border {
  border: 1px solid rgba(0, 0, 0, 0.14);
  border-radius: 6px;
  padding: 12px 16px;
  background: rgba(0, 0, 0, 0.035);
}
@media (prefers-color-scheme: dark) {
  .note-embed-border {
    border-color: rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.05);
  }
}
.note-embed-inner.read-mode-content > :first-child {
  margin-top: 0;
}
.note-embed-inner.read-mode-content > :last-child {
  margin-bottom: 0;
}
.note-embed-stub {
  margin: 0.55em 0;
  font-size: 0.92em;
  opacity: 0.92;
}
.note-embed-stub-label {
  font-weight: 600;
}
.note-embed-stub--missing {
  font-style: italic;
}
`;

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

/**
 * Full HTML document matching read-mode Markdown rendering (including transclusion).
 * @param {string} title
 * @param {string} markdownBody
 * @param {{ rootNoteId?: number | null }} [options]
 */
export async function buildStandaloneReadModeHtml(title, markdownBody, options = {}) {
  const safeTitle = escapeHtml(title || 'Note');
  const innerHtml = await renderTransclusionMarkdownToHtml(markdownBody || '', {
    rootNoteId: options.rootNoteId ?? null,
  });
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${safeTitle}</title>
<style>${READ_MODE_EXPORT_CSS}</style>
</head>
<body>
<article class="read-mode-content">
${innerHtml}
</article>
</body>
</html>`;
}

/**
 * Normalise a string for use as a print-job / save-dialog filename stem.
 * @param {string} base
 */
function sanitisePrintBasename(base) {
  const s = String(base ?? '')
    .replace(/[/\\:*?"<>|]/g, '-')
    .trim()
    .replace(/\s+/g, ' ');
  return s || 'note';
}

/**
 * Open the OS print dialog for an HTML document (user may choose Save as PDF).
 *
 * Chromium/WebView2 derives the default "Save as PDF" filename from the **document
 * that owns the print job**. Printing from a hidden iframe still attributes the job
 * to the host window — hence `grimoire.pdf` from index.html. We therefore print
 * from a minimal auxiliary window whose `document.title` is the note name.
 *
 * If `window.open` fails (popup blocked), falls back to iframe print while briefly
 * setting the host `document.title` so the dialog may pick up the note name.
 *
 * @param {string} html full document from {@link buildStandaloneReadModeHtml}
 * @param {string} [suggestedBaseName] note title / filename stem (no extension)
 */
export function printStandaloneHtml(html, suggestedBaseName) {
  const safeBase = sanitisePrintBasename(suggestedBaseName);

  const printFromIframeWithHostTitle = () => {
    const prevHostTitle = document.title;
    document.title = safeBase;

    const iframe = document.createElement('iframe');
    iframe.setAttribute('aria-hidden', 'true');
    iframe.style.cssText =
      'position:fixed;right:0;bottom:0;width:0;height:0;border:0;opacity:0;pointer-events:none';
    document.body.appendChild(iframe);

    const doc = iframe.contentDocument;
    if (!doc) {
      document.title = prevHostTitle;
      iframe.remove();
      return;
    }

    const win = iframe.contentWindow;
    doc.open();
    doc.write(html);
    doc.close();
    try {
      doc.title = safeBase;
    } catch {
      /* ignore */
    }

    let cleaned = false;
    const restoreHost = () => {
      if (cleaned) return;
      cleaned = true;
      document.title = prevHostTitle;
      iframe.remove();
    };

    const runPrint = () => {
      try {
        win?.focus();
        win?.print();
      } finally {
        window.addEventListener('afterprint', restoreHost, { once: true });
        setTimeout(restoreHost, 4000);
      }
    };

    if (doc.readyState === 'complete') {
      requestAnimationFrame(runPrint);
    } else {
      iframe.onload = () => runPrint();
    }
  };

  let popup = null;
  try {
    popup = window.open(
      'about:blank',
      '_blank',
      'popup=yes,width=1200,height=800,left=-32000,top=-32000',
    );
  } catch {
    popup = null;
  }

  if (!popup || popup.closed) {
    printFromIframeWithHostTitle();
    return;
  }

  try {
    popup.document.open();
    popup.document.write(html);
    popup.document.close();
    popup.document.title = safeBase;
  } catch {
    try {
      popup.close();
    } catch {
      /* ignore */
    }
    printFromIframeWithHostTitle();
    return;
  }

  let closed = false;
  const closePopup = () => {
    if (closed) return;
    closed = true;
    try {
      popup.close();
    } catch {
      /* ignore */
    }
  };

  const doPrint = () => {
    try {
      popup.focus();
      popup.print();
    } finally {
      popup.addEventListener('afterprint', closePopup, { once: true });
      setTimeout(closePopup, 4000);
    }
  };

  if (popup.document.readyState === 'complete') {
    requestAnimationFrame(doPrint);
  } else {
    popup.onload = () => doPrint();
  }
}
