import { describe, it, expect } from 'vitest';
import { assessChatModelHardware, inferChatModelResourceTier } from './chatModelHardware.js';

describe('inferChatModelResourceTier', () => {
  it('classifies curated llama3:70b as xlarge', () => {
    expect(inferChatModelResourceTier('llama3:70b')).toBe('xlarge');
  });

  it('infers 70b from custom tag', () => {
    expect(inferChatModelResourceTier('foo:70b-instruct')).toBe('xlarge');
  });

  it('classifies VRAM-sweet-spot presets', () => {
    expect(inferChatModelResourceTier('llama3.1:8b')).toBe('medium');
    expect(inferChatModelResourceTier('qwen2.5:14b')).toBe('heavy');
  });
});

describe('assessChatModelHardware', () => {
  it('returns ok for light model on modest RAM', () => {
    const r = assessChatModelHardware('phi3', {
      ramTotalMb: 12 * 1024,
      capability: 'full',
      gpus: [{ vramTotalMb: 8192, isUnifiedMemory: false }],
    });
    expect(r.level).toBe('ok');
    expect(r.lines.length).toBe(0);
  });

  it('flags xlarge model on low RAM', () => {
    const r = assessChatModelHardware('llama3:70b', {
      ramTotalMb: 8 * 1024,
      capability: 'embeddingOnly',
      gpus: [{ vramTotalMb: 4096, isUnifiedMemory: false }],
    });
    expect(r.level).toBe('severe');
    expect(r.lines.length).toBeGreaterThan(0);
  });
});
