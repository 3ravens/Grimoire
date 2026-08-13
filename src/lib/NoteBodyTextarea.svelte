<script>
    import { getContext } from "svelte";

    const ns = getContext("ns");

    let {
        noteId,
        value = $bindable(""),
        onkeydown = () => {},
    } = $props();

    /** @type {HTMLTextAreaElement | null} */
    let el = $state(null);

    $effect(() => {
        ns.editorTextareaEl = el;
        return () => {
            if (ns.editorTextareaEl === el) {
                ns.editorTextareaEl = null;
            }
        };
    });

    $effect(() => {
        if (!el) return;
        const next = value ?? "";
        if (el.value === next) return;
        const top = el.scrollTop;
        el.value = next;
        el.scrollTop = top;
    });

    function handleInput(e) {
        value = /** @type {HTMLTextAreaElement} */ (e.currentTarget).value;
        ns.markDirty();
    }
</script>

{#key noteId}
    <textarea
        class="content-area"
        bind:this={el}
        oninput={handleInput}
        {onkeydown}
        placeholder="Write your note…"
    ></textarea>
{/key}
