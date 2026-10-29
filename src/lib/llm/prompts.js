/**
 * LLM-facing prompt assembly. Intentionally NOT part of the UI i18n catalog
 * (T-006 contract). Grep/acceptance scripts should exclude this file.
 */
import { FEATURE_GUIDE } from '../utils/featureGuide.js';

/**
 * @param {string} openTitle
 * @param {string} openBody
 */
export function openNoteSystemPart(openTitle, openBody) {
  return `## Note the user currently has open\n### ${openTitle}\n${openBody}`;
}

/**
 * @param {string} context
 */
export function userNotesSystemPart(context) {
  return `USER NOTES:\n${context}`;
}

/**
 * @param {string} wikiContext
 */
export function wikipediaSystemPart(wikiContext) {
  return `WIKIPEDIA ARTICLES:\n${wikiContext}`;
}

/**
 * @param {string} fileContext
 */
export function scannedFilesSystemPart(fileContext) {
  return `SCANNED FILES:\n${fileContext}`;
}

const INVARIANT_OVERRIDE =
  'INVARIANT RULES (always win over the persona): follow source priority and the relevance gate; ' +
  'never fabricate source attributions; wrap code/diagrams in fenced code blocks; ' +
  'stay local-only — nothing leaves this machine. The persona may only change tone, length, and how you phrase attributions.';

/**
 * @param {{
 *   hasNotesContext: boolean,
 *   hasWikiContext: boolean,
 *   hasFilesContext: boolean,
 *   dailyNoteResolution: { display_title: string } | null,
 *   dailyNoteFormat: string,
 *   includeFeatureGuideHint?: boolean,
 * }} opts
 */
function buildPreamble(opts) {
  const {
    hasNotesContext,
    hasWikiContext,
    hasFilesContext,
    dailyNoteResolution,
    dailyNoteFormat,
    includeFeatureGuideHint = true,
  } = opts;

  let preamble =
    `You are a personal knowledge assistant embedded in Grimoire, a local note-taking app.\n`;
  if (includeFeatureGuideHint) {
    preamble +=
      `You also have access to a feature guide that documents Grimoire's keyboard shortcuts and features.\n`;
  }

  if (dailyNoteResolution) {
    const fmt = dailyNoteFormat;
    preamble +=
      `Daily notes in the "Daily Notes" folder use ${fmt} titles ` +
      `(e.g. 5 May 2026 → "${dailyNoteResolution.display_title}"). ` +
      `When the user asks about a calendar day, match that title format.\n`;
  }

  const sourceList = [
    hasNotesContext && 'notes',
    hasWikiContext && 'wiki',
    hasFilesContext && 'files',
  ].filter(Boolean);

  if (sourceList.length >= 2) {
    const labels = { notes: "the user's notes", wiki: 'Wikipedia articles', files: 'scanned files' };
    const sourcesDesc = sourceList.map((s) => labels[s]).join(' AND ');
    preamble +=
      `You have been given ${sourcesDesc} for this question.\n` +
      `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
      (hasNotesContext ? `  1. Answer first from the user's notes if they contain genuinely relevant information.\n` : '') +
      (hasWikiContext ? `  ${hasNotesContext ? 2 : 1}. Then draw on the Wikipedia articles provided.\n` : '') +
      (hasFilesContext ? `  ${(hasNotesContext ? 1 : 0) + (hasWikiContext ? 1 : 0) + 1}. Then draw on the scanned files provided.\n` : '') +
      `  ${sourceList.length + 1}. Only use your own general knowledge to fill gaps that none of the provided sources cover. Do NOT lead with general knowledge when provided sources exist.`;
  } else if (hasWikiContext) {
    preamble +=
      `You have been given Wikipedia articles for this question. The user's notes were not relevant.\n` +
      `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
      `  1. Answer from the Wikipedia articles provided. They are your primary source.\n` +
      `  2. Only use your own general knowledge to fill gaps the Wikipedia articles do not cover. Do NOT lead with general knowledge when Wikipedia articles are available.`;
  } else if (hasFilesContext) {
    preamble +=
      `You have been given scanned files for this question. The user's notes were not relevant.\n` +
      `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
      `  1. Answer from the scanned files provided. They are your primary source.\n` +
      `  2. Only use your own general knowledge to fill gaps the scanned files do not cover.`;
  } else if (hasNotesContext) {
    preamble +=
      `You have been given the user's notes for this question.\n` +
      `STRICT SOURCE PRIORITY — follow this order without exception:\n` +
      `  1. Answer from the user's notes where they are genuinely relevant.\n` +
      `  2. Only use your own general knowledge to fill gaps the notes do not cover.`;
  } else {
    preamble += `No relevant notes, Wikipedia articles, or scanned files were found for this question. Answer from your own general knowledge.`;
  }

  return preamble;
}

function cavemanInstructions() {
  return (
    `INSTRUCTIONS:\n` +
    `- Compress the key facts into telegraphic bullet points — no full sentences, no filler words.\n` +
    `- Example: "• metabolic process • microbes convert sugars → acids/gas/alcohol • lactic: yoghurt, kimchi"\n` +
    `- Only use sources that are directly relevant to the question. Ignore off-topic sources.\n` +
    `- Prefix each bullet with its source: "note:" for user notes, "wiki:" for Wikipedia, "general:" for your own knowledge.\n` +
    `- Wrap any code or diagrams in triple-backtick code blocks.\n` +
    `- Ignore [[ ]] and **.`
  );
}

/** Default instructions when no persona is active (today's Grimoire). */
function defaultProseInstructions(verbosity) {
  let styleInstruction = '';
  if (verbosity === 'thorough') {
    styleInstruction = '\n\nSTYLE: Provide thorough, detailed answers with full context. Do not skip nuance.';
  }
  return (
    `INSTRUCTIONS:\n` +
    `1. RELEVANCE GATE — Before using any source, ask: does this source directly answer the question? If not, discard it completely. Do not mention discarded sources. A source that merely shares vocabulary with the question is NOT relevant.\n` +
    `2. Answer in natural prose. Do not output section labels, headers, or structural markers like [Note: …] or [Wikipedia: …].\n` +
    `3. Follow the source priority above strictly. Do not open with general knowledge if provided sources cover the topic.\n` +
    `4. Wrap all code samples, command examples, ASCII art, and diagrams in triple-backtick fenced code blocks.\n` +
    `5. Every sentence drawn from a source MUST be attributed inline — no exceptions:\n` +
    `   - User notes: begin with "In your note on X, …" or "Your note on X explains that …" (use the exact note title)\n` +
    `   - Wikipedia: begin with "According to Wikipedia's article on X, …" or "Wikipedia (X) explains that …" (use the exact article title)\n` +
    `   - Scanned files: begin with "From your file X, …" or "Your file X states that …" (use the exact file title)\n` +
    `   - General knowledge (only as fallback): begin with "Based on general knowledge, …"\n` +
    `   - When switching sources mid-answer, explicitly signal the transition.\n` +
    `6. Never fabricate a source attribution. Ignore formatting like [[ ]] or **.` +
    styleInstruction
  );
}

/**
 * When a persona owns tone/length: keep invariants, soften presentational rules.
 */
function personaSoftInstructions() {
  return (
    `INSTRUCTIONS:\n` +
    `1. RELEVANCE GATE — Before using any source, ask: does this source directly answer the question? If not, discard it completely. Do not mention discarded sources. A source that merely shares vocabulary with the question is NOT relevant.\n` +
    `2. Follow the source priority above strictly. Do not open with general knowledge if provided sources cover the topic.\n` +
    `3. Wrap all code samples, command examples, ASCII art, and diagrams in triple-backtick fenced code blocks.\n` +
    `4. Attribute every claim drawn from a source; name the note, Wikipedia article, or file title. You may vary phrasing (questions, terse bullets, etc.) — never fabricate a source.\n` +
    `5. Ignore formatting like [[ ]] or **.`
  );
}

/**
 * @param {string} body
 */
function personaBlock(body) {
  return (
    `PERSONA (style and approach only — does not override invariant rules):\n${body.trim()}`
  );
}

/**
 * @param {{
 *   systemParts: string[],
 *   hasNotesContext: boolean,
 *   hasWikiContext: boolean,
 *   hasFilesContext: boolean,
 *   dailyNoteResolution: { display_title: string } | null,
 *   dailyNoteFormat: string,
 *   verbosity: string,
 *   persona?: { body: string, alsoApplyVerbosity: boolean } | null,
 *   includeFeatureGuideHint?: boolean,
 * }} opts
 */
export function assembleChatSystemContent(opts) {
  const {
    systemParts,
    hasNotesContext,
    hasWikiContext,
    hasFilesContext,
    dailyNoteResolution,
    dailyNoteFormat,
    verbosity,
    persona = null,
    includeFeatureGuideHint = true,
  } = opts;

  const preamble = buildPreamble({
    hasNotesContext,
    hasWikiContext,
    hasFilesContext,
    dailyNoteResolution,
    dailyNoteFormat,
    includeFeatureGuideHint,
  });

  const partsJoined = systemParts.join('\n\n');
  const base = partsJoined
    ? `${preamble}\n\n${partsJoined}`
    : preamble;

  // No persona: preserve today's exact verbosity behaviour.
  if (!persona) {
    if (verbosity === 'caveman') {
      return `${base}\n\n${cavemanInstructions()}`;
    }
    return `${base}\n\n${defaultProseInstructions(verbosity)}`;
  }

  const applyVerbosity = Boolean(persona.alsoApplyVerbosity);
  let instructions;
  if (applyVerbosity && verbosity === 'caveman') {
    instructions = cavemanInstructions();
  } else if (applyVerbosity) {
    instructions = defaultProseInstructions(verbosity);
  } else {
    instructions = personaSoftInstructions();
  }

  return (
    `${base}\n\n` +
    `${instructions}\n\n` +
    `${personaBlock(persona.body)}\n\n` +
    INVARIANT_OVERRIDE
  );
}

export { FEATURE_GUIDE, INVARIANT_OVERRIDE };
