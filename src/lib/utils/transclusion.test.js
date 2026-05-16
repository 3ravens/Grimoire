import { describe, it, expect } from 'vitest';
import { splitMarkdownByEmbeds } from './transclusion.js';

describe('splitMarkdownByEmbeds', () => {
  it('extracts embed titles', () => {
    const parts = splitMarkdownByEmbeds('Intro ![[My Note]] outro');
    const embed = parts.find((p) => p.type === 'embed');
    expect(embed).toBeDefined();
    expect(embed.value).toBe('My Note');
  });

  it('does not treat embed syntax inside fenced code as embed', () => {
    const md = "```\n![[ignored]]\n```";
    const parts = splitMarkdownByEmbeds(md);
    expect(parts.every((p) => p.type === 'text')).toBe(true);
    expect(parts[0].value).toContain('![[');
  });

  it('handles missing closing brackets as plain text', () => {
    const parts = splitMarkdownByEmbeds('Hello ![[no close');
    expect(parts.every((p) => p.type === 'text')).toBe(true);
  });
});
