import { describe, it, expect } from 'vitest';
import {
  assembleChatSystemContent,
  INVARIANT_OVERRIDE,
} from './prompts.js';

const baseOpts = {
  systemParts: ['USER NOTES:\n[Note: "A"]\nhello'],
  hasNotesContext: true,
  hasWikiContext: false,
  hasFilesContext: false,
  dailyNoteResolution: null,
  dailyNoteFormat: 'j M Y',
};

describe('assembleChatSystemContent', () => {
  it('None + concise matches today (no STYLE, no PERSONA, no override)', () => {
    const content = assembleChatSystemContent({ ...baseOpts, verbosity: 'concise' });
    expect(content).toContain('RELEVANCE GATE');
    expect(content).toContain('Answer in natural prose');
    expect(content).not.toContain('STYLE:');
    expect(content).not.toContain('PERSONA');
    expect(content).not.toContain(INVARIANT_OVERRIDE);
  });

  it('None + thorough adds STYLE', () => {
    const content = assembleChatSystemContent({ ...baseOpts, verbosity: 'thorough' });
    expect(content).toContain('STYLE: Provide thorough');
    expect(content).not.toContain('PERSONA');
  });

  it('None + caveman uses telegraphic instructions', () => {
    const content = assembleChatSystemContent({ ...baseOpts, verbosity: 'caveman' });
    expect(content).toContain('telegraphic bullet points');
    expect(content).not.toContain('RELEVANCE GATE');
    expect(content).not.toContain('PERSONA');
  });

  it('persona with flag off omits STYLE/Caveman and softens presentational rules', () => {
    const content = assembleChatSystemContent({
      ...baseOpts,
      verbosity: 'thorough',
      persona: { body: 'Be a Socratic tutor.', alsoApplyVerbosity: false },
    });
    expect(content).toContain('PERSONA (style and approach only');
    expect(content).toContain('Be a Socratic tutor.');
    expect(content).toContain(INVARIANT_OVERRIDE);
    expect(content).not.toContain('STYLE:');
    expect(content).not.toContain('telegraphic bullet points');
    expect(content).toContain('You may vary phrasing');
    expect(content).not.toContain('Answer in natural prose');
  });

  it('persona with flag off ignores caveman verbosity setting', () => {
    const content = assembleChatSystemContent({
      ...baseOpts,
      verbosity: 'caveman',
      persona: { body: 'Blunt.', alsoApplyVerbosity: false },
    });
    expect(content).not.toContain('telegraphic bullet points');
    expect(content).toContain('Blunt.');
  });

  it('persona with alsoApplyVerbosity + caveman keeps caveman path and persona', () => {
    const content = assembleChatSystemContent({
      ...baseOpts,
      verbosity: 'caveman',
      persona: { body: 'Still be helpful.', alsoApplyVerbosity: true },
    });
    expect(content).toContain('telegraphic bullet points');
    expect(content).toContain('Still be helpful.');
    expect(content).toContain(INVARIANT_OVERRIDE);
  });

  it('persona with alsoApplyVerbosity + thorough keeps STYLE', () => {
    const content = assembleChatSystemContent({
      ...baseOpts,
      verbosity: 'thorough',
      persona: { body: 'Extra tone.', alsoApplyVerbosity: true },
    });
    expect(content).toContain('STYLE: Provide thorough');
    expect(content).toContain('Answer in natural prose');
    expect(content).toContain('Extra tone.');
  });

  it('omits feature-guide hint when includeFeatureGuideHint is false', () => {
    const content = assembleChatSystemContent({
      ...baseOpts,
      systemParts: [],
      hasNotesContext: false,
      verbosity: 'concise',
      includeFeatureGuideHint: false,
      persona: { body: 'Tone only.', alsoApplyVerbosity: false },
    });
    expect(content).not.toContain('feature guide');
    expect(content).toContain('Tone only.');
  });

  it('persona with empty systemParts still returns system content', () => {
    const content = assembleChatSystemContent({
      ...baseOpts,
      systemParts: [],
      hasNotesContext: false,
      verbosity: 'concise',
      persona: { body: 'Cite notes only bias.', alsoApplyVerbosity: false },
    });
    expect(content.length).toBeGreaterThan(0);
    expect(content).toContain('Cite notes only bias.');
    expect(content).toContain('No relevant notes');
    expect(content).toContain(INVARIANT_OVERRIDE);
  });
});
