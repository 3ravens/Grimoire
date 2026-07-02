// Copyright (C) 2026 Wim Palland
// This file is part of Grimoire — licensed under GPL-3.0 or later.

import DOMPurify from 'dompurify';

/** Sanitize markdown-rendered note HTML for read mode / transclusion. */
export function sanitizeNoteHtml(html) {
  return DOMPurify.sanitize(html ?? '', {
    USE_PROFILES: { html: true },
  });
}

/** Sanitize Wikipedia article HTML while preserving in-app navigation attributes. */
export function sanitizeWikipediaHtml(html) {
  return DOMPurify.sanitize(html ?? '', {
    USE_PROFILES: { html: true },
    ADD_ATTR: ['data-wiki-path', 'data-external'],
  });
}

/** Sanitize FTS snippet HTML (highlight tags only). */
export function sanitizeSearchSnippet(html) {
  return DOMPurify.sanitize(html ?? '', {
    ALLOWED_TAGS: ['b', 'mark', 'strong', 'em'],
    ALLOWED_ATTR: [],
  });
}
