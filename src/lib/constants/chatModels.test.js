import { describe, it, expect } from 'vitest';
import { statsForAnyModelId, findCuratedChatModel, isEmbeddingModelId } from './chatModels.js';

describe('statsForAnyModelId', () => {
  it('returns curated stats for llama3.2', () => {
    const s = statsForAnyModelId('llama3.2');
    expect(s.statsShort).toContain('GB');
    expect(s.statsDetail.length).toBeGreaterThan(20);
  });

  it('uses heuristics for custom ids', () => {
    const s = statsForAnyModelId('foo:70b');
    expect(s.statsShort).toMatch(/40B|45|GB/i);
  });

  it('findCuratedChatModel matches tagged name', () => {
    expect(findCuratedChatModel('mistral:latest')?.value).toBe('mistral');
  });

  it('findCuratedChatModel matches llama3.1:8b tag', () => {
    expect(findCuratedChatModel('llama3.1:8b:latest')?.value).toBe('llama3.1:8b');
  });

  it('treats embedding presets as embedding ids', () => {
    expect(isEmbeddingModelId('nomic-embed-text')).toBe(true);
    expect(isEmbeddingModelId('nomic-embed-text:latest')).toBe(true);
    expect(isEmbeddingModelId('mistral')).toBe(false);
  });
});
