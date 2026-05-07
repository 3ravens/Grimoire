import { describe, it, expect } from 'vitest';
import { FEATURE_GUIDE } from './featureGuide.js';

describe('FEATURE_GUIDE', () => {
  it('documents core product areas', () => {
    expect(FEATURE_GUIDE).toContain('Grimoire');
    expect(FEATURE_GUIDE).toContain('RAG');
    expect(FEATURE_GUIDE).toContain('Vault');
  });
});
