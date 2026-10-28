import { describe, it, expect } from 'vitest';
import { FOLDER_PIN_MIME, isFolderPinDrag } from './folderPinDrag.js';

describe('folderPinDrag', () => {
  it('detects the private MIME', () => {
    expect(
      isFolderPinDrag({ types: [FOLDER_PIN_MIME, 'text/plain'] }),
    ).toBe(true);
    expect(isFolderPinDrag({ types: ['text/plain'] })).toBe(false);
    expect(isFolderPinDrag(null)).toBe(false);
  });
});
