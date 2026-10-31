/**
 * Move the line block covering the selection up (−1) or down (+1).
 * Preserves relative selection offsets within the moved block.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {1 | -1} direction
 * @returns {{ value: string, selectionStart: number, selectionEnd: number } | null}
 */
export function moveEditorLines(value, start, end, direction) {
  const from = Math.max(0, Math.min(start, end));
  const to = Math.max(0, Math.max(start, end));
  const { lines, starts } = parseLines(value);
  const first = lineIndexAt(starts, from);
  const last = lineIndexAt(starts, selectionEndLinePos(value, from, to));

  if (direction < 0) {
    if (first === 0) return null;
    const next = lines.slice();
    const block = next.splice(first, last - first + 1);
    next.splice(first - 1, 0, ...block);
    const nextValue = joinParsed(next, value);
    const delta = starts[first] - starts[first - 1];
    return {
      value: nextValue,
      selectionStart: from - delta,
      selectionEnd: to - delta,
    };
  }

  if (last >= lines.length - 1) return null;
  const next = lines.slice();
  const block = next.splice(first, last - first + 1);
  next.splice(first + 1, 0, ...block);
  const nextValue = joinParsed(next, value);
  const delta = lines[last + 1].length + 1;
  return {
    value: nextValue,
    selectionStart: from + delta,
    selectionEnd: to + delta,
  };
}

/**
 * Duplicate the line block covering the selection above (−1) or below (+1).
 * Selection moves onto the new copy.
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {1 | -1} direction
 * @returns {{ value: string, selectionStart: number, selectionEnd: number }}
 */
export function duplicateEditorLines(value, start, end, direction) {
  const from = Math.max(0, Math.min(start, end));
  const to = Math.max(0, Math.max(start, end));
  const { lines, starts } = parseLines(value);
  const first = lineIndexAt(starts, from);
  const last = lineIndexAt(starts, selectionEndLinePos(value, from, to));
  const block = lines.slice(first, last + 1);
  const relStart = from - starts[first];
  const relEnd = to - starts[first];
  const next = lines.slice();

  if (direction < 0) {
    next.splice(first, 0, ...block);
    const nextValue = joinParsed(next, value);
    return {
      value: nextValue,
      selectionStart: starts[first] + relStart,
      selectionEnd: starts[first] + relEnd,
    };
  }

  next.splice(last + 1, 0, ...block);
  const nextValue = joinParsed(next, value);
  const { starts: newStarts } = parseLines(nextValue);
  const copyFirst = last + 1;
  return {
    value: nextValue,
    selectionStart: newStarts[copyFirst] + relStart,
    selectionEnd: newStarts[copyFirst] + relEnd,
  };
}

/**
 * Split into logical lines. A trailing newline yields a final empty line.
 *
 * @param {string} value
 */
function parseLines(value) {
  const lines = value.split("\n");
  const starts = [];
  let offset = 0;
  for (let i = 0; i < lines.length; i++) {
    starts.push(offset);
    offset += lines[i].length + (i < lines.length - 1 ? 1 : 0);
  }
  return { lines, starts };
}

/** @param {string[]} lines */
function joinParsed(lines) {
  return lines.join("\n");
}

/**
 * Position used to determine the last selected line.
 * @param {string} value
 * @param {number} from
 * @param {number} to
 */
function selectionEndLinePos(value, from, to) {
  if (to > from && value[to - 1] === "\n") return to - 1;
  return to;
}

/**
 * @param {number[]} starts
 * @param {number} pos
 */
function lineIndexAt(starts, pos) {
  for (let i = starts.length - 1; i >= 0; i--) {
    if (pos >= starts[i]) return i;
  }
  return 0;
}


