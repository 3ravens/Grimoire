/** @typedef {{ type: 'add' | 'remove' | 'unchanged', lines: string[] }} DiffHunk */
/** @typedef {{ type: 'equal' | 'remove' | 'add', text: string }} InlineSegment */
/**
 * @typedef {DiffHunk | {
 *   type: 'modified',
 *   oldLine: string,
 *   newLine: string,
 *   oldSegments: InlineSegment[],
 *   newSegments: InlineSegment[],
 * }} DisplayDiffHunk
 */

/**
 * Compute a line-level diff between two texts.
 * Returns an array of hunks, each representing a contiguous block of
 * added, removed, or unchanged lines.
 *
 * @param {string} original
 * @param {string} improved
 * @returns {DiffHunk[]}
 */
export function computeDiff(original, improved) {
  const a = original.split('\n');
  const b = improved.split('\n');
  return diffLines(a, b);
}

/**
 * Merge adjacent single-line remove + add hunks into `modified` hunks with
 * word-level inline segments for clearer UI (readonly diff / history).
 * Improve-mode hunks stay line-based if you skip this.
 *
 * @param {DiffHunk[]} hunks
 * @returns {DisplayDiffHunk[]}
 */
export function coalesceSingleLineChangeHunks(hunks) {
  const out = [];
  for (let i = 0; i < hunks.length; i++) {
    const h = hunks[i];
    const next = hunks[i + 1];
    if (
      h.type === 'remove' &&
      next &&
      next.type === 'add' &&
      h.lines.length === 1 &&
      next.lines.length === 1
    ) {
      const oldLine = h.lines[0];
      const newLine = next.lines[0];
      const { oldSegments, newSegments } = diffTokenSegments(oldLine, newLine);
      out.push({
        type: 'modified',
        oldLine,
        newLine,
        oldSegments,
        newSegments,
      });
      i++;
    } else {
      out.push(h);
    }
  }
  return out;
}

/**
 * Apply accepted hunks to the original text, producing a partially-improved result.
 * `accepted` is a Set of hunk indices.
 *
 * @param {DiffHunk[]} hunks
 * @param {Set<number>} acceptedIndices
 * @param {string} original
 * @param {string} improved
 * @returns {string}
 */
export function applyAcceptedHunks(hunks, acceptedIndices, original, improved) {
  const a = original.split('\n');
  const b = improved.split('\n');
  let ai = 0;
  const result = [];

  for (let hi = 0; hi < hunks.length; hi++) {
    const hunk = hunks[hi];
    if (hunk.type === 'unchanged') {
      result.push(...hunk.lines);
      ai += hunk.lines.length;
    } else if (hunk.type === 'remove') {
      if (acceptedIndices.has(hi)) {
        ai += hunk.lines.length;
      } else {
        result.push(...hunk.lines);
        ai += hunk.lines.length;
      }
    } else if (hunk.type === 'add') {
      if (acceptedIndices.has(hi)) {
        result.push(...hunk.lines);
      }
    }
  }

  return result.join('\n');
}

// ── Internal: LCS-based line diff ──────────────────────────────────────────

/**
 * Myers-like diff using LCS on lines.
 * Returns groups of (added/removed/unchanged) line blocks.
 *
 * @param {string[]} a original lines
 * @param {string[]} b improved lines
 * @returns {DiffHunk[]}
 */
function diffLines(a, b) {
  const lcs = computeLCS(a, b);
  const hunks = [];
  let ai = 0, bi = 0, li = 0;

  while (ai < a.length || bi < b.length) {
    if (li < lcs.length) {
      const lcsLine = lcs[li];
      // Find where this LCS line appears next in both arrays.
      let nextA = ai;
      while (nextA < a.length && a[nextA] !== lcsLine) nextA++;
      let nextB = bi;
      while (nextB < b.length && b[nextB] !== lcsLine) nextB++;

      // Emit removed lines (in a but not in b) before the next common line.
      if (nextA > ai) {
        hunks.push({ type: 'remove', lines: a.slice(ai, nextA) });
      }
      // Emit added lines (in b but not in a) before the next common line.
      if (nextB > bi) {
        hunks.push({ type: 'add', lines: b.slice(bi, nextB) });
      }

      // Emit the common line.
      hunks.push({ type: 'unchanged', lines: [lcsLine] });

      ai = nextA + 1;
      bi = nextB + 1;
      li++;
    } else {
      // No more LCS — remaining lines are removes and/or adds.
      if (ai < a.length) {
        hunks.push({ type: 'remove', lines: a.slice(ai) });
      }
      if (bi < b.length) {
        hunks.push({ type: 'add', lines: b.slice(bi) });
      }
      break;
    }
  }

  return mergeAdjacentHunks(hunks);
}

/**
 * Merge adjacent hunks of the same type for cleaner display.
 *
 * @param {DiffHunk[]} hunks
 * @returns {DiffHunk[]}
 */
function mergeAdjacentHunks(hunks) {
  const merged = [];
  for (const h of hunks) {
    const last = merged[merged.length - 1];
    if (last && last.type === h.type) {
      last.lines.push(...h.lines);
    } else {
      merged.push(h);
    }
  }
  return merged;
}

/**
 * Compute the Longest Common Subsequence of two line arrays.
 * Uses the standard O(n*m) DP approach with backtracking.
 *
 * @param {string[]} a
 * @param {string[]} b
 * @returns {string[]}
 */
function computeLCS(a, b) {
  const n = a.length;
  const m = b.length;
  // dp[i][j] = LCS length of a[0..i-1] and b[0..j-1]
  const dp = new Array(n + 1);
  for (let i = 0; i <= n; i++) {
    dp[i] = new Uint16Array(m + 1);
  }

  for (let i = 1; i <= n; i++) {
    const ai = a[i - 1];
    const row = dp[i];
    const prev = dp[i - 1];
    for (let j = 1; j <= m; j++) {
      if (ai === b[j - 1]) {
        row[j] = prev[j - 1] + 1;
      } else {
        row[j] = Math.max(prev[j], row[j - 1]);
      }
    }
  }

  // Backtrack to reconstruct the LCS.
  const result = [];
  let i = n, j = m;
  while (i > 0 && j > 0) {
    if (a[i - 1] === b[j - 1]) {
      result.push(a[i - 1]);
      i--; j--;
    } else if (dp[i - 1][j] > dp[i][j - 1]) {
      i--;
    } else {
      j--;
    }
  }
  result.reverse();
  return result;
}

// ── Word-level segments for single-line edits ─────────────────────────────

/** Split a line into words and whitespace chunks (preserves spaces/newlines in tokens). */
function tokenizeLine(s) {
  if (!s) return [];
  return s.split(/(\s+)/).filter((t) => t.length > 0);
}

function mergeAdjacentSegments(segs) {
  const out = [];
  for (const s of segs) {
    const last = out[out.length - 1];
    if (last && last.type === s.type) {
      last.text += s.text;
    } else {
      out.push({ type: s.type, text: s.text });
    }
  }
  return out;
}

/**
 * Diff two strings at token (word + whitespace) granularity for inline highlighting.
 *
 * @param {string} oldLine
 * @param {string} newLine
 * @returns {{ oldSegments: InlineSegment[], newSegments: InlineSegment[] }}
 */
export function diffTokenSegments(oldLine, newLine) {
  const ta = tokenizeLine(oldLine);
  const tb = tokenizeLine(newLine);
  const lcs = computeLCS(ta, tb);
  /** @type {InlineSegment[]} */
  const oldSeg = [];
  /** @type {InlineSegment[]} */
  const newSeg = [];
  let ai = 0;
  let bi = 0;
  let li = 0;

  while (ai < ta.length || bi < tb.length) {
    if (li < lcs.length) {
      const token = lcs[li];
      let na = ai;
      while (na < ta.length && ta[na] !== token) na++;
      let nb = bi;
      while (nb < tb.length && tb[nb] !== token) nb++;

      if (na > ai) {
        oldSeg.push({ type: 'remove', text: ta.slice(ai, na).join('') });
      }
      if (nb > bi) {
        newSeg.push({ type: 'add', text: tb.slice(bi, nb).join('') });
      }

      oldSeg.push({ type: 'equal', text: token });
      newSeg.push({ type: 'equal', text: token });

      ai = na + 1;
      bi = nb + 1;
      li++;
    } else {
      if (ai < ta.length) {
        oldSeg.push({ type: 'remove', text: ta.slice(ai).join('') });
      }
      if (bi < tb.length) {
        newSeg.push({ type: 'add', text: tb.slice(bi).join('') });
      }
      break;
    }
  }

  return {
    oldSegments: mergeAdjacentSegments(oldSeg),
    newSegments: mergeAdjacentSegments(newSeg),
  };
}
