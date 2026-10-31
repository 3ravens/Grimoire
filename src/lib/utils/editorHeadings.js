/**
 * Cycle ATX heading level on each line in the selection.
 * paragraph → # → ## → ### → #### → paragraph (reverse opposite).
 * Setext headings are left alone (no underline rewrite).
 *
 * @param {string} value
 * @param {number} start
 * @param {number} end
 * @param {{ reverse?: boolean }} [opts]
 * @returns {{ value: string, selectionStart: number, selectionEnd: number }}
 */
export function cycleHeadingLevel(value, start, end, { reverse = false } = {}) {
  const from = Math.max(0, Math.min(start, end));
  const to = Math.max(0, Math.max(start, end));
  const blockStart = lineStart(value, from);
  let endIndex = to;
  if (to > from && value[to - 1] === "\n") {
    endIndex = to - 1;
  }
  const blockEnd = lineEnd(value, Math.max(endIndex, from));
  const block = value.slice(blockStart, blockEnd);
  const lines = block.split("\n");
  const nextLines = lines.map((line) => cycleLine(line, reverse));
  const newBlock = nextLines.join("\n");

  const mappedStart = mapOffsetThroughCycle(block, newBlock, from - blockStart);
  const mappedEnd = mapOffsetThroughCycle(block, newBlock, to - blockStart);

  return {
    value: value.slice(0, blockStart) + newBlock + value.slice(blockEnd),
    selectionStart: blockStart + mappedStart,
    selectionEnd: blockStart + mappedEnd,
  };
}

/**
 * @param {string} oldBlock
 * @param {string} newBlock
 * @param {number} rel
 */
function mapOffsetThroughCycle(oldBlock, newBlock, rel) {
  const oldLines = oldBlock.split("\n");
  const newLines = newBlock.split("\n");
  let oldPos = 0;
  let newPos = 0;
  for (let i = 0; i < oldLines.length; i++) {
    const oldLine = oldLines[i];
    const newLine = newLines[i] ?? oldLine;
    const oldLineEnd = oldPos + oldLine.length;
    if (rel <= oldLineEnd || i === oldLines.length - 1) {
      const offsetInLine = Math.max(0, Math.min(rel - oldPos, oldLine.length));
      const oldPrefix = atxPrefixLength(oldLine);
      const newPrefix = atxPrefixLength(newLine);
      if (offsetInLine <= oldPrefix) {
        // Caret at line start on a plain paragraph → land after new markers.
        if (oldPrefix === 0 && offsetInLine === 0 && newPrefix > 0) {
          return newPos + newPrefix;
        }
        return newPos + Math.min(offsetInLine, newPrefix);
      }
      return newPos + newPrefix + (offsetInLine - oldPrefix);
    }
    oldPos = oldLineEnd + 1;
    newPos += newLine.length + 1;
  }
  return newBlock.length;
}

/**
 * @param {string} line
 * @param {boolean} reverse
 */
function cycleLine(line, reverse) {
  const hadCr = line.endsWith("\r");
  const raw = hadCr ? line.slice(0, -1) : line;
  const cr = hadCr ? "\r" : "";

  const m = /^( {0,3})(#{1,6})(\s+)(.*)$/.exec(raw);
  if (!m) {
    const indentMatch = /^( {0,3})(.*)$/.exec(raw);
    const indent = indentMatch?.[1] ?? "";
    const content = indentMatch?.[2] ?? raw;
    if (reverse) {
      return `${indent}#### ${content}${cr}`;
    }
    return `${indent}# ${content}${cr}`;
  }

  const indent = m[1];
  const level = m[2].length;
  const content = m[4];

  // Only cycle H1–H4; leave H5+ alone.
  if (level > 4) return line;

  if (!reverse) {
    if (level >= 4) {
      return `${indent}${content}${cr}`;
    }
    return `${indent}${"#".repeat(level + 1)} ${content}${cr}`;
  }

  if (level <= 1) {
    return `${indent}${content}${cr}`;
  }
  return `${indent}${"#".repeat(level - 1)} ${content}${cr}`;
}

/** @param {string} line */
function atxPrefixLength(line) {
  const m = /^( {0,3}#{1,6}\s+)/.exec(line.replace(/\r$/, ""));
  return m ? m[1].length : 0;
}

/** @param {string} value @param {number} index */
function lineStart(value, index) {
  const i = value.lastIndexOf("\n", Math.max(0, index - 1));
  return i === -1 ? 0 : i + 1;
}

/** @param {string} value @param {number} index */
function lineEnd(value, index) {
  const i = value.indexOf("\n", index);
  return i === -1 ? value.length : i;
}

