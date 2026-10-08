<script lang="ts">
  import { lib } from './store.svelte';

  interface Props {
    /** Currently selected artist names, in order. */
    value: string[];
    onChange: (names: string[]) => void;
  }

  let { value, onChange }: Props = $props();

  let query = $state('');
  let inputEl: HTMLInputElement | undefined = $state(undefined);
  let highlighted = $state(0);
  let focused = $state(false);

  // Case-insensitive index over the already-loaded artist list (`lib.artists`),
  // keyed by lowercased name for O(1) exact-match lookups. Rebuilt only when the
  // underlying artist list changes, not on every keystroke.
  let nameIndex = $derived.by(() => {
    const map = new Map<string, { id: number; name: string }>();
    for (const a of lib.artists) map.set(a.name.toLowerCase(), { id: a.id, name: a.name });
    return map;
  });

  const selectedLower = $derived(new Set(value.map((v) => v.toLowerCase())));

  let suggestions = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return [];
    const out: { id: number; name: string }[] = [];
    for (const a of lib.artists) {
      if (selectedLower.has(a.name.toLowerCase())) continue;
      if (a.name.toLowerCase().includes(q)) {
        out.push({ id: a.id, name: a.name });
        if (out.length >= 8) break;
      }
    }
    return out;
  });

  // Whether the current query exactly matches an existing artist (so "Enter"
  // should select it instead of creating a duplicate).
  let exactMatch = $derived(nameIndex.get(query.trim().toLowerCase()) ?? null);

  function addArtist(name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    if (selectedLower.has(trimmed.toLowerCase())) { query = ''; return; }
    onChange([...value, trimmed]);
    query = '';
    highlighted = 0;
  }

  function removeArtist(name: string) {
    onChange(value.filter((v) => v !== name));
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      if (suggestions.length) { e.preventDefault(); highlighted = (highlighted + 1) % suggestions.length; }
      return;
    }
    if (e.key === 'ArrowUp') {
      if (suggestions.length) { e.preventDefault(); highlighted = (highlighted - 1 + suggestions.length) % suggestions.length; }
      return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      e.stopPropagation();
      if (suggestions.length) {
        addArtist(suggestions[Math.min(highlighted, suggestions.length - 1)].name);
      } else if (exactMatch) {
        addArtist(exactMatch.name);
      } else if (query.trim()) {
        // No existing artist matches — create a new one on save (the backend
        // find-or-creates artists by name via `create_or_ignore`).
        addArtist(query);
      }
      return;
    }
    if (e.key === 'Backspace' && !query && value.length > 0) {
      removeArtist(value[value.length - 1]);
      return;
    }
    if (e.key === 'Escape') {
      query = '';
      inputEl?.blur();
    }
  }
</script>

<div class="artist-multiselect" class:focused>
  <div class="chips-row" onclick={() => inputEl?.focus()} role="presentation">
    {#each value as name (name)}
      <span class="artist-chip">
        <span class="chip-label">{name}</span>
        <button
          type="button"
          class="chip-remove"
          onclick={(e) => { e.stopPropagation(); removeArtist(name); }}
          aria-label={`Remove ${name}`}
        ><i class="pxi pxi-close" aria-hidden="true"></i></button>
      </span>
    {/each}
    <input
      bind:this={inputEl}
      bind:value={query}
      onkeydown={handleKeydown}
      onfocus={() => (focused = true)}
      onblur={() => { focused = false; }}
      placeholder={value.length === 0 ? 'Artist 1, Artist 2…' : ''}
      class="chip-input"
    />
  </div>

  {#if focused && (suggestions.length > 0 || query.trim())}
    <ul class="suggestions">
      {#each suggestions as s, i (s.id)}
        <li>
          <button
            type="button"
            class="suggestion-item"
            class:active={i === highlighted}
            onmousedown={(e) => { e.preventDefault(); addArtist(s.name); }}
            onmouseenter={() => (highlighted = i)}
          >{s.name}</button>
        </li>
      {/each}
      {#if query.trim() && !exactMatch}
        <li>
          <button
            type="button"
            class="suggestion-item suggestion-create"
            onmousedown={(e) => { e.preventDefault(); addArtist(query); }}
          ><i class="pxi pxi-plus" aria-hidden="true"></i>Create "{query.trim()}"</button>
        </li>
      {/if}
    </ul>
  {/if}
</div>

<style>
  .artist-multiselect { position: relative; }

  /* Reads as one text field: global input skin and focus ring. */
  .chips-row {
    display: flex; flex-wrap: wrap; align-items: center; gap: 6px;
    min-height: 42px; padding: 5px 8px;
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius-control); cursor: text;
    transition: border-color var(--motion-fast) var(--ease-out), box-shadow var(--motion-fast) var(--ease-out);
  }
  .artist-multiselect.focused .chips-row { border-color: var(--accent); box-shadow: var(--focus-ring); }

  .artist-chip {
    display: inline-flex; align-items: center; gap: 2px;
    min-height: 26px; padding: 0 2px 0 8px;
    border: 1px solid var(--border-strong); border-radius: var(--radius-chip);
    background: var(--bg); color: var(--text-bright); white-space: nowrap;
  }
  .chip-label { font-family: var(--font-mono); font-size: 12px; letter-spacing: 0.02em; }
  .chip-remove {
    display: grid; place-items: center; width: 22px; height: 22px; padding: 0;
    border: none; border-radius: 4px; background: none;
    color: var(--muted-2); font-size: 16px; cursor: pointer;
  }
  .chip-remove:hover { color: var(--error); background: var(--surface-2); }

  .chip-input {
    flex: 1; min-width: 6rem; padding: 4px 0;
    background: none; border: none; outline: none;
    color: var(--text); font-size: 14px; font-family: inherit;
  }
  .chip-input::placeholder { color: var(--muted-2); }

  /* Suggestions: a floating panel. */
  .suggestions {
    position: absolute; top: calc(100% + 6px); left: 0; right: 0; z-index: 20;
    list-style: none; margin: 0; padding: 4px;
    background: var(--float); border: 1px solid var(--float-border); border-radius: var(--radius-card);
    box-shadow: var(--float-shadow);
    max-height: 220px; overflow-y: auto; overscroll-behavior: contain;
  }
  .suggestion-item {
    display: flex; align-items: center; gap: 8px; width: 100%; min-height: 36px;
    padding: 0 10px; border: none; border-radius: var(--radius-chip);
    background: none; color: var(--text); font-size: 14px; font-family: inherit; text-align: left; cursor: pointer;
  }
  .suggestion-item.active, .suggestion-item:hover { background: var(--surface-2); color: var(--text-bright); }
  .suggestion-item .pxi { font-size: 16px; }
  .suggestion-create { color: var(--accent); }

  @media (max-width: 860px), (hover: none) and (pointer: coarse) {
    .chips-row { min-height: 44px; }
    .artist-chip { max-width: 100%; white-space: normal; overflow-wrap: anywhere; }
    .chip-input { min-width: 0; flex-basis: 100%; }
    .chip-remove { min-width: 44px; min-height: 44px; }
    .suggestion-item { min-height: 44px; }
  }
</style>
