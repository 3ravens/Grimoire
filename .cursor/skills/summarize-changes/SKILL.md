---
name: summarize-changes
description: Generate a short, concise summary for uncommitted changes for a non-technical audience. Use when the user asks for changelog-style summaries, plain-English change descriptions, or summaries of git diffs/uncommitted changes.
disable-model-invocation: true
---

# Summarize Changes

You are an expert at summarizing software changes for a non-technical audience.

Analyze the uncommitted changes and write a short, high-level summary of what changed and why it matters. Think changelog entry, not commit log.

## Rules
- 2–4 sentences maximum
- Plain English, no jargon, no bullet points
- Focus on what the user experiences, not implementation details
- Do not mention file names, function names, or code

Output the summary, then do a linebreak and add a one-sentence description of the changes.
