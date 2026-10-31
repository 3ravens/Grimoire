<script>
    import { getContext } from "svelte";
    import { t } from "./i18n/t.js";

    const ns = getContext("ns");

    let {
        noteId,
        value = $bindable(""),
        wrap = "soft",
        onkeydown = () => {},
        onpaste = () => {},
        onbeforeinput = () => {},
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
        {wrap}
        oninput={handleInput}
        {onbeforeinput}
        {onkeydown}
        {onpaste}
        placeholder={t("notes.writePlaceholder")}
    ></textarea>
{/key}
