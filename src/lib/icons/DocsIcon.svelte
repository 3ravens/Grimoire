<!-- Open book — activity bar docs. -->
<script>
    /** @type {{ class?: string, size?: number }} */
    let { class: className = "", size = 13 } = $props();
</script>

<svg
    width={size}
    height={size}
    class={className}
    viewBox="0 0 15 15"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
>
    <path d="M3.5 2.5h4a2 2 0 0 1 2 2v8.5H4a1 1 0 0 1-1-1v-8a1 1 0 0 1 .5-.87z" />
    <path d="M7.5 2.5H11a1.5 1.5 0 0 1 1.5 1.5v8.5" />
    <line x1="5.5" y1="5.5" x2="8.5" y2="5.5" />
    <line x1="5.5" y1="7.5" x2="8.5" y2="7.5" />
    <line x1="5.5" y1="9.5" x2="7.5" y2="9.5" />
</svg>
