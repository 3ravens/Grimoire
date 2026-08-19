import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}));

import { createChatSessionService, CLEAR_CONVERSATION_LABEL } from './chatSessionService.svelte.js';

describe('chatSessionService', () => {
  let session;

  beforeEach(() => {
    session = createChatSessionService();
  });

  it('label is exactly "Clear conversation"', () => {
    expect(CLEAR_CONVERSATION_LABEL).toBe('Clear conversation');
  });

  it('starts with empty state', () => {
    expect(session.messages).toEqual([]);
    expect(session.isLoading).toBe(false);
    expect(session.sourcesUsed).toEqual([]);
    expect(session.wikiSourcesUsed).toEqual([]);
    expect(session.notesError).toBe('');
    expect(session.streamError).toBe('');
  });

  it('clearConversation resets messages and crumbs', () => {
    session.messages = [{ role: 'user', content: 'hi' }, { role: 'assistant', content: 'hello' }];
    session.sourcesUsed = ['Note A'];
    session.wikiSourcesUsed = [{ title: 'W' }];
    session.notesError = 'err';
    session.streamError = 'fail';

    session.clearConversation();

    expect(session.messages).toEqual([]);
    expect(session.sourcesUsed).toEqual([]);
    expect(session.wikiSourcesUsed).toEqual([]);
    expect(session.notesError).toBe('');
    expect(session.streamError).toBe('');
  });

  it('clearConversation is a no-op while loading without force', () => {
    session.messages = [{ role: 'user', content: 'hi' }];
    session.isLoading = true;

    session.clearConversation();

    expect(session.messages).toHaveLength(1);
  });

  it('clearConversation with force works while loading', () => {
    session.messages = [{ role: 'user', content: 'hi' }];
    session.isLoading = true;
    const genBefore = session.generation;

    session.clearConversation({ force: true });

    expect(session.messages).toEqual([]);
    expect(session.isLoading).toBe(false);
    expect(session.generation).toBe(genBefore + 1);
  });

  it('clearConversation increments generation', () => {
    const genBefore = session.generation;
    session.clearConversation();
    expect(session.generation).toBe(genBefore + 1);
  });
});
