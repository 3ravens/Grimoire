import { listen } from '@tauri-apps/api/event';

/**
 * Shared chat session state — owns conversation turns and streaming lifecycle
 * so that sidebar and Chat tab share the same history without remount loss.
 */
export function createChatSessionService() {
  let messages = $state([]);
  let isLoading = $state(false);
  let sourcesUsed = $state([]);
  let wikiSourcesUsed = $state([]);
  let notesError = $state('');
  let streamError = $state('');

  /** Monotonic counter; late tokens from a previous generation are ignored. */
  let generation = $state(0);
  /** Unlisten handle for the active `chat:token` listener, if any. */
  let unlisten = null;

  /**
   * Discard all conversation turns and last-turn crumbs.
   * No-op while streaming unless `force` is true (used by vault lock).
   * NOTE: if chat persistence ships later, this must confirm and only clear the active thread.
   */
  function clearConversation({ force = false } = {}) {
    if (isLoading && !force) return;
    generation++;
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    messages = [];
    isLoading = false;
    sourcesUsed = [];
    wikiSourcesUsed = [];
    notesError = '';
    streamError = '';
  }

  /**
   * Begin a new streaming response. Pushes a placeholder assistant message
   * and subscribes to `chat:token` on the service (survives Chat remount).
   * @param {Array<{role: string, content: string}>} history - turns ending with the user message
   */
  async function beginStream(history) {
    generation++;
    const gen = generation;
    isLoading = true;
    streamError = '';
    messages = [...history, { role: 'assistant', content: '' }];

    if (unlisten) {
      unlisten();
      unlisten = null;
    }

    unlisten = await listen('chat:token', (event) => {
      if (generation !== gen) return;
      messages = messages.map((m, i) =>
        i === messages.length - 1
          ? { ...m, content: m.content + event.payload }
          : m
      );
    });

    return gen;
  }

  /** Finalize a stream (success or error). */
  function endStream() {
    if (unlisten) {
      unlisten();
      unlisten = null;
    }
    isLoading = false;
  }

  return {
    get messages() { return messages; },
    set messages(v) { messages = v; },
    get isLoading() { return isLoading; },
    set isLoading(v) { isLoading = v; },
    get sourcesUsed() { return sourcesUsed; },
    set sourcesUsed(v) { sourcesUsed = v; },
    get wikiSourcesUsed() { return wikiSourcesUsed; },
    set wikiSourcesUsed(v) { wikiSourcesUsed = v; },
    get notesError() { return notesError; },
    set notesError(v) { notesError = v; },
    get streamError() { return streamError; },
    set streamError(v) { streamError = v; },
    get generation() { return generation; },
    clearConversation,
    beginStream,
    endStream,
  };
}

/** Label used by all Clear conversation UI surfaces. */
export const CLEAR_CONVERSATION_LABEL = 'Clear conversation';
