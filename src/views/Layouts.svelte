<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { _ } from "svelte-i18n";
  import { Check, RefreshCw, Save, Trash2, LayoutDashboard } from "lucide-svelte";
  import { api } from "../lib/api";
  import { askConfirm, showError, showInfo } from "../lib/dialogs";
  import { busy, describeOutput, hasStep, runJob } from "../lib/jobs";
  import { DEFAULT_LAYOUT, LAYOUTS, isBuiltIn, type CustomLayout } from "../lib/layouts";
  import SaveLayoutDialog from "../components/SaveLayoutDialog.svelte";

  // The first page of the 2.x program: the four layouts with a picture each,
  // one of them chosen, and the buttons to apply it and to reload the desktop.
  // The layouts the user saves come after the four, and the desktop as it is
  // can be saved as a new one. The one chosen when the window opens is the one
  // applied last, which the program keeps in the same file the 2.x version
  // used.

  const t = (key: string, values?: Record<string, string | number>) => get(_)(key, { values });

  let selected: string = DEFAULT_LAYOUT;
  let custom: CustomLayout[] = [];
  let saving = false;

  async function loadCustom() {
    try {
      custom = await api.invoke<CustomLayout[]>("list_custom_layouts");
    } catch (error) {
      console.error("Reading the layouts of the user failed", error);
      custom = [];
    }
  }

  onMount(async () => {
    await loadCustom();
    try {
      const saved = await api.invoke<string | null>("get_layout");
      if (saved && (isBuiltIn(saved) || custom.some((layout) => layout.id === saved))) selected = saved;
    } catch (error) {
      console.error("Reading the layout failed", error);
    }
  });

  const nameOf = (id: string) => (isBuiltIn(id) ? t(`layouts.${id}`) : (custom.find((layout) => layout.id === id)?.name ?? id));

  // What the job said that the person has to read even when it worked: an
  // extension that could not be turned on yet, one to install by hand, a
  // setting that did not change.
  const NOTES = ["enableFailed", "manual", "packagesFailed", "settingFailed", "noBuild", "saveFailed"];

  async function apply() {
    if ($busy) return;
    const name = nameOf(selected);
    const result = await runJob("apply_layout", { id: selected });
    if (!result.success) {
      await showError(t("errors.title"), t("errors.apply"), describeOutput(result.output));
      return;
    }
    if (NOTES.some((key) => hasStep(result.output, key))) {
      await showInfo(t("notices.title"), t("notices.appliedNotes", { name }), describeOutput(result.output));
    } else {
      await showInfo(t("notices.title"), t("notices.applied", { name }));
    }
  }

  async function reload() {
    if ($busy) return;
    const result = await runJob("reload_desktop");
    if (!result.success) await showError(t("errors.title"), t("errors.reload"), describeOutput(result.output));
  }

  // Saves the desktop as it is now. A name that is already there asks before
  // it replaces that layout.
  async function save(name: string) {
    saving = false;
    const known = custom.find((layout) => layout.name.trim().toLowerCase() === name.toLowerCase());
    if (known && !(await askConfirm(t("layouts.replaceTitle"), t("layouts.replaceBody", { name: known.name }), t("layouts.replace")))) return;
    const result = await runJob("save_current_layout", { name });
    if (!result.success) {
      await showError(t("errors.title"), t("errors.save"), describeOutput(result.output));
      return;
    }
    await loadCustom();
    const saved = custom.find((layout) => layout.name.trim().toLowerCase() === name.toLowerCase());
    if (saved) selected = saved.id;
    await showInfo(t("notices.title"), t("notices.saved", { name }));
  }

  async function remove(layout: CustomLayout) {
    if ($busy) return;
    if (!(await askConfirm(t("layouts.deleteTitle"), t("layouts.deleteBody", { name: layout.name }), t("layouts.delete"), true))) return;
    let worked = false;
    try {
      worked = await api.invoke<boolean>("delete_custom_layout", { id: layout.id });
    } catch (error) {
      console.error("Deleting the layout failed", error);
    }
    await loadCustom();
    if (selected === layout.id) selected = DEFAULT_LAYOUT;
    if (!worked) await showError(t("errors.title"), t("errors.delete"));
  }

  const primaryButton = "flex items-center gap-2 px-5 py-2.5 rounded bg-primary hover:bg-primary-hover text-black text-[0.84rem] font-semibold disabled:opacity-50 disabled:pointer-events-none";
  const secondaryButton =
    "flex items-center gap-2 px-5 py-2.5 rounded border border-border bg-surface-2 text-text-bright hover:border-primary text-[0.84rem] font-semibold disabled:opacity-50 disabled:pointer-events-none";
  // The four layouts in two rows of two, as in the 2.x window. With the window
  // opening at 700 x 700 they fit without the scroll bar: each picture
  // (200 x 150) is shown at about 195 wide. The layouts the user saves go on
  // after them, so the page scrolls only when there are many.
  const cardClass = (on: boolean) => `group flex flex-col gap-1.5 rounded-lg border p-2 text-left transition-all bg-panel ${on ? "border-primary ring-2 ring-primary/50 shadow-lg" : "border-border hover:border-primary/60"}`;
</script>

<div class="flex flex-col gap-5">
  <div class="grid grid-cols-2 gap-3 w-full max-w-[400px] mx-auto" role="radiogroup" aria-label={$_("tabs.layout")}>
    {#each LAYOUTS as id (id)}
      <button role="radio" aria-checked={selected === id} class={cardClass(selected === id)} on:click={() => (selected = id)}>
        <span class="flex items-center gap-1.5 text-[0.78rem] font-semibold {selected === id ? 'text-primary' : 'text-text-bright'}">
          <span class="grid h-3.5 w-3.5 shrink-0 place-items-center rounded-full border {selected === id ? 'border-primary bg-primary text-black' : 'border-border'}">
            {#if selected === id}<Check size={9} strokeWidth={3} />{/if}
          </span>
          {$_(`layouts.${id}`)}
        </span>
        <img
          src="/assets/layouts/{id}preview.svg"
          alt=""
          class="w-full rounded-md transition-opacity {selected === id ? 'opacity-100' : 'opacity-70 group-hover:opacity-90'}"
          draggable="false"
        />
      </button>
    {/each}

    {#each custom as layout (layout.id)}
      <!-- A box and not a button: it holds the button that deletes the layout. -->
      <div
        role="radio"
        tabindex="0"
        aria-checked={selected === layout.id}
        class="{cardClass(selected === layout.id)} cursor-pointer"
        on:click={() => (selected = layout.id)}
        on:keydown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), (selected = layout.id))}
      >
        <span class="flex items-center gap-1.5 text-[0.78rem] font-semibold {selected === layout.id ? 'text-primary' : 'text-text-bright'}">
          <span class="grid h-3.5 w-3.5 shrink-0 place-items-center rounded-full border {selected === layout.id ? 'border-primary bg-primary text-black' : 'border-border'}">
            {#if selected === layout.id}<Check size={9} strokeWidth={3} />{/if}
          </span>
          <span class="min-w-0 flex-1 truncate" title={layout.name}>{layout.name}</span>
          <button
            class="shrink-0 rounded p-1 text-text-muted hover:bg-surface-2 hover:text-danger disabled:opacity-50"
            title={$_("layouts.delete")}
            aria-label={$_("layouts.delete")}
            disabled={$busy}
            on:click|stopPropagation={() => remove(layout)}
          >
            <Trash2 size={13} />
          </button>
        </span>
        <div class="grid aspect-[4/3] w-full place-items-center rounded-md border border-border bg-surface-2 text-primary transition-opacity {selected === layout.id ? 'opacity-100' : 'opacity-70 group-hover:opacity-90'}">
          <LayoutDashboard size={34} strokeWidth={1.4} />
        </div>
      </div>
    {/each}
  </div>

  <div class="flex flex-wrap items-center justify-center gap-3">
    <button class={secondaryButton} disabled={$busy} on:click={reload}>
      <RefreshCw size={15} />{$_("layouts.reload")}
    </button>
    <button class={secondaryButton} disabled={$busy} on:click={() => (saving = true)}>
      <Save size={15} />{$_("layouts.save")}
    </button>
    <button class={primaryButton} disabled={$busy} on:click={apply}>
      <Check size={15} />{$_("layouts.apply")}
    </button>
  </div>
</div>

{#if saving}
  <SaveLayoutDialog onClose={() => (saving = false)} onSave={save} />
{/if}
