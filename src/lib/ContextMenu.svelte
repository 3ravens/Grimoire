<!-- Copyright (C) 2026 Wim Palland
This file is part of Grimoire — licensed under GPL-3.0 or later. -->

<script>
  import { tick } from 'svelte';

  let { x, y, items, onClose } = $props();

  // Clamp to viewport so the main menu never clips off-screen.
  const clampedX = $derived(Math.min(x, window.innerWidth  - 174));
  const clampedY = $derived(Math.min(y, window.innerHeight - items.length * 28 - 16));

  // Whether submenus open to the right (enough room) or flip left.
  const subRight = $derived(clampedX + 340 < window.innerWidth);

  // ── Keyboard navigation state ─────────────────────────────────────────────

  // Indices (in items[]) of non-divider items, for up/down navigation.
  const navIndices = $derived(
    items.reduce((/** @type {number[]} */ acc, item, i) => {
      if (!item.divider) acc.push(i);
      return acc;
    }, [])
  );

  // Current position within navIndices (index of focused main-menu item).
  let focusPos = $state(0);

  // items[] index of whichever submenu parent is currently open via keyboard (-1 = none).
  let submenuOpenIdx = $state(-1);

  // Position within the open submenu's non-divider items.
  let subFocusPos = $state(0);

  // Navigable indices within the current open submenu.
  const subNavIndices = $derived(
    submenuOpenIdx >= 0 && items[submenuOpenIdx]?.submenu
      ? items[submenuOpenIdx].submenu.reduce((/** @type {number[]} */ acc, sub, i) => {
          if (!sub.divider) acc.push(i);
          return acc;
        }, [])
      : []
  );

  let menuEl = $state(null);

  // Auto-focus the first menuitem when the menu mounts.
  $effect(() => {
    if (!menuEl) return;
    menuEl.querySelector('[role="menuitem"]:not([disabled])')?.focus();
  });

  async function handleMainKeydown(e) {
    // When focus is inside an open submenu, only handle Escape/Left to close it.
    if (submenuOpenIdx >= 0) {
      if (e.key === 'Escape' || e.key === 'ArrowLeft') {
        e.preventDefault();
        e.stopPropagation();
        submenuOpenIdx = -1;
        await tick();
        menuEl?.querySelector(`[data-menu-idx="${navIndices[focusPos]}"]`)?.focus();
      }
      return;
    }

    const len = navIndices.length;
    if (len === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      focusPos = (focusPos + 1) % len;
      await tick();
      menuEl?.querySelector(`[data-menu-idx="${navIndices[focusPos]}"]`)?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      focusPos = (focusPos - 1 + len) % len;
      await tick();
      menuEl?.querySelector(`[data-menu-idx="${navIndices[focusPos]}"]`)?.focus();
    } else if (e.key === 'ArrowRight' || e.key === 'Enter' || e.key === ' ') {
      const item = items[navIndices[focusPos]];
      if (item?.submenu) {
        e.preventDefault();
        submenuOpenIdx = navIndices[focusPos];
        subFocusPos = 0;
        await tick();
        menuEl?.querySelector('[data-sub-idx="0"]')?.focus();
      } else if ((e.key === 'Enter' || e.key === ' ') && item?.action && !item.disabled) {
        e.preventDefault();
        item.action();
        onClose();
      }
    } else if (e.key === 'Escape') {
      onClose();
    }
  }

  async function handleSubKeydown(e) {
    const len = subNavIndices.length;
    if (len === 0) return;

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      subFocusPos = (subFocusPos + 1) % len;
      await tick();
      menuEl?.querySelector(`[data-sub-idx="${subNavIndices[subFocusPos]}"]`)?.focus();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      subFocusPos = (subFocusPos - 1 + len) % len;
      await tick();
      menuEl?.querySelector(`[data-sub-idx="${subNavIndices[subFocusPos]}"]`)?.focus();
    } else if (e.key === 'ArrowLeft' || e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      submenuOpenIdx = -1;
      await tick();
      menuEl?.querySelector(`[data-menu-idx="${navIndices[focusPos]}"]`)?.focus();
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      const subItem = items[submenuOpenIdx]?.submenu?.[subNavIndices[subFocusPos]];
      if (subItem?.action && !subItem.disabled) {
        subItem.action();
        onClose();
      }
    }
  }
</script>


<!-- Transparent backdrop — catches any click outside the menu -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="ctx-backdrop" onmousedown={onClose}></div>

<ul
  bind:this={menuEl}
  class="ctx-menu"
  style="left: {clampedX}px; top: {clampedY}px"
  role="menu"
  onkeydown={handleMainKeydown}
>
  {#each items as item, idx}
    {#if item.divider}
      <li class="ctx-divider" role="separator"></li>
    {:else if item.submenu}
      <li class="ctx-item-wrap" class:open-kb={submenuOpenIdx === idx} role="none">
        <button
          class="ctx-item ctx-item-has-sub"
          role="menuitem"
          aria-haspopup="true"
          aria-expanded={submenuOpenIdx === idx}
          tabindex={navIndices[focusPos] === idx ? 0 : -1}
          data-menu-idx={idx}
          onmouseenter={() => { submenuOpenIdx = -1; focusPos = navIndices.indexOf(idx); }}
        >
          {item.label}
          <span class="ctx-arrow">›</span>
        </button>
        <ul class="ctx-submenu" class:sub-left={!subRight} role="menu" onkeydown={handleSubKeydown}>
          {#each item.submenu as sub, si}
            {#if sub.divider}
              <li class="ctx-divider" role="separator"></li>
            {:else}
              <li role="none">
                <button
                  class="ctx-item"
                  role="menuitem"
                  tabindex={submenuOpenIdx === idx && subNavIndices[subFocusPos] === si ? 0 : -1}
                  data-sub-idx={si}
                  onmousedown={(e) => { e.stopPropagation(); sub.action(); onClose(); }}
                >{sub.label}</button>
              </li>
            {/if}
          {/each}
        </ul>
      </li>
    {:else}
      <li role="none">
        <button
          class="ctx-item"
          class:danger={item.danger}
          class:disabled={item.disabled}
          role="menuitem"
          disabled={item.disabled}
          tabindex={navIndices[focusPos] === idx ? 0 : -1}
          data-menu-idx={idx}
          onmouseenter={() => { submenuOpenIdx = -1; focusPos = navIndices.indexOf(idx); }}
          onmousedown={(e) => {
            e.stopPropagation();
            if (!item.disabled) { item.action(); onClose(); }
          }}
        >{item.label}</button>
      </li>
    {/if}
  {/each}
</ul>

<style>
  @import './styles/context-menu.css';
</style>
