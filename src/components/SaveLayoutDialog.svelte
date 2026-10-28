<script lang="ts">
  import { _ } from "svelte-i18n";
  import Modal from "./Modal.svelte";
  import { autofocus } from "../lib/autofocus";
  import { MAX_NAME_CHARS, validLayoutName } from "../lib/layouts";

  // Asks for the name of the layout to save. It does not save anything: it
  // hands the name over and the page does the rest.
  export let onClose: () => void;
  export let onSave: (name: string) => void;

  let name = "";
  $: valid = validLayoutName(name);
  $: showProblem = name.trim().length > 0 && !valid;

  function submit() {
    if (valid) onSave(name.trim());
  }
</script>

<Modal variant="confirm" title={$_("layouts.saveTitle")} showFooter={false} maxWidth={460} {onClose}>
  <p>{$_("layouts.saveBody")}</p>
  <label for="layout-name" class="mt-3 block text-[0.78rem] text-text-muted">{$_("layouts.nameLabel")}</label>
  <input
    id="layout-name"
    type="text"
    maxlength={MAX_NAME_CHARS}
    class="mt-1 w-full rounded border border-border bg-surface-2 px-3 py-2 text-[0.85rem] text-text-bright outline-none focus-visible:border-primary"
    bind:value={name}
    use:autofocus
    on:keydown={(e) => e.key === "Enter" && submit()}
  />
  {#if showProblem}
    <p class="mt-1.5 text-[0.75rem] text-danger">{$_("errors.name")}</p>
  {/if}
  <div class="mt-4 flex justify-end gap-2">
    <button class="px-4 py-2 rounded border border-border text-text hover:bg-surface-2 text-[0.85rem]" on:click={onClose}>
      {$_("common.cancel")}
    </button>
    <button
      class="px-5 py-2 rounded bg-primary hover:bg-primary-hover text-text-bright text-[0.85rem] font-semibold disabled:opacity-50 disabled:pointer-events-none"
      disabled={!valid}
      on:click={submit}
    >
      {$_("common.save")}
    </button>
  </div>
</Modal>
