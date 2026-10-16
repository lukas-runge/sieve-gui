<script lang="ts">
  // Thin CodeMirror 6 wrapper for Sieve code (concept §6, decision D3).
  import { onMount } from "svelte";
  import { EditorState, Compartment } from "@codemirror/state";
  import { EditorView, keymap, lineNumbers, highlightActiveLine, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
  import { StreamLanguage, syntaxHighlighting, defaultHighlightStyle, bracketMatching } from "@codemirror/language";
  import { sieve } from "@codemirror/legacy-modes/mode/sieve";

  let {
    value,
    readonly = false,
    label,
    onchange,
  }: { value: string; readonly?: boolean; label: string; onchange?: (value: string) => void } = $props();

  let host: HTMLDivElement;
  let view: EditorView | undefined;
  const editable = new Compartment();

  const theme = EditorView.theme({
    "&": {
      fontSize: "0.85rem",
      backgroundColor: "var(--color-base-100)",
      color: "var(--color-base-content)",
      borderRadius: "var(--radius-field)",
    },
    ".cm-content": { fontFamily: "var(--font-mono)", caretColor: "var(--color-base-content)" },
    ".cm-gutters": {
      backgroundColor: "var(--color-base-200)",
      color: "color-mix(in oklab, var(--color-base-content) 45%, transparent)",
      border: "none",
    },
    ".cm-activeLine": { backgroundColor: "color-mix(in oklab, var(--color-primary) 6%, transparent)" },
    "&.cm-focused": { outline: "2px solid var(--color-primary)", outlineOffset: "1px" },
  });

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          history(),
          drawSelection(),
          highlightActiveLine(),
          bracketMatching(),
          StreamLanguage.define(sieve),
          syntaxHighlighting(defaultHighlightStyle),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          theme,
          editable.of([EditorState.readOnly.of(readonly), EditorView.editable.of(!readonly)]),
          // The read-only preview sits in a narrow panel: wrap instead of scrolling sideways.
          readonly ? EditorView.lineWrapping : [],
          EditorView.contentAttributes.of({ "aria-label": label }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange?.(u.state.doc.toString());
          }),
        ],
      }),
    });
    return () => view?.destroy();
  });

  // Follow outside changes (e.g. the generated preview) without losing the
  // cursor when the text is the same.
  $effect(() => {
    const next = value;
    if (view && view.state.doc.toString() !== next) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: next } });
    }
  });
</script>

<div bind:this={host} class="overflow-hidden rounded-field border border-base-300"></div>
