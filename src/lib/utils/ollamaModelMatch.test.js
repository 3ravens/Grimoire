import { describe, it, expect } from 'vitest';
import { ollamaInstalledMatchesRequest, firstInstalledFullName } from './ollamaModelMatch.js';

describe('ollamaInstalledMatchesRequest', () => {
  it('matches base without tag to any variant', () => {
    expect(ollamaInstalledMatchesRequest('phi3:latest', 'phi3')).toBe(true);
    expect(ollamaInstalledMatchesRequest('phi3.5:latest', 'phi3')).toBe(false);
  });

  it('requires tag match when requested includes tag', () => {
    expect(ollamaInstalledMatchesRequest('phi3:latest', 'phi3:latest')).toBe(true);
    expect(ollamaInstalledMatchesRequest('phi3:latest', 'phi3:3.8b')).toBe(false);
  });

  it('does not conflate llama3 and llama3.2', () => {
    expect(ollamaInstalledMatchesRequest('llama3.2:latest', 'llama3')).toBe(false);
    expect(ollamaInstalledMatchesRequest('llama3.2:latest', 'llama3.2')).toBe(true);
  });
});

describe('firstInstalledFullName', () => {
  it('returns first matching full name', () => {
    const installed = ['mistral:latest', 'phi3:latest'];
    expect(firstInstalledFullName('mistral', installed)).toBe('mistral:latest');
  });
});
