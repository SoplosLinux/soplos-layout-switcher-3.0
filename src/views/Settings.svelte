<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { _ } from "svelte-i18n";
  import { api } from "../lib/api";
  import { showError } from "../lib/dialogs";
  import { busy, describeOutput, runJob } from "../lib/jobs";
  import { TOOLS, type Switches, type ToolId } from "../lib/layouts";

  // The second page of the 2.x program: the two switches (the system tray and
  // the desktop icons, which are GNOME Shell extensions) and the buttons that
  // open other tools. The switches show what is on now, which the shell says.

  const t = (key: string, values?: Record<string, string | number>) => get(_)(key, { values });

  let switches: Switches = { tray: false, desktopIcons: false };
  let ready = false;
  let changing = false;

  async function readSwitches() {
    try {
      switches = await api.invoke<Switches>("get_switches");
    } catch (error) {
      console.error("Reading the extensions failed", error);
    }
    ready = true;
  }

  onMount(readSwitches);

  async function toggle(kind: "tray" | "desktopIcons") {
    if (changing || $busy) return;
    changing = true;
    const enabled = !switches[kind];
    let worked = false;
    try {
      worked = await api.invoke<boolean>("set_switch", { kind, enabled });
    } catch (error) {
      console.error("Changing the extension failed", error);
    }
    // Whatever happened, the switch shows what the shell says now.
    await readSwitches();
    changing = false;
    if (!worked) await showError(t("errors.title"), t("errors.switch"));
  }

  async function open(tool: ToolId) {
    if ($busy) return;
    const result = await runJob("open_tool_job", { tool });
    if (!result.success) await showError(t("errors.title"), t("errors.tool"), describeOutput(result.output));
  }

  const SWITCHES = [
    { kind: "tray" as const, label: "settings.tray" },
    { kind: "desktopIcons" as const, label: "settings.desktopIcons" },
  ];

  const row = "flex items-center justify-between gap-4 rounded-lg border border-border bg-panel px-4 py-3";
  const openButton =
    "px-4 py-1.5 rounded border border-border bg-surface-2 text-text-bright hover:border-primary text-[0.8rem] font-semibold disabled:opacity-50 disabled:pointer-events-none";
</script>

<div class="grid grid-cols-1 gap-3 max-w-[600px] w-full mx-auto sm:grid-cols-2">
  {#each SWITCHES as item (item.kind)}
    <div class={row}>
      <span class="text-[0.85rem] text-text-bright">{$_(item.label)}</span>
      <button
        role="switch"
        aria-checked={switches[item.kind]}
        aria-label={$_(item.label)}
        class="relative h-6 w-11 shrink-0 rounded-full border transition-colors disabled:opacity-50 {switches[item.kind] ? 'border-primary bg-primary' : 'border-border bg-surface-2'}"
        disabled={!ready || changing || $busy}
        on:click={() => toggle(item.kind)}
      >
        <span class="absolute top-0.5 rounded-full bg-white shadow transition-all {switches[item.kind] ? 'left-[22px]' : 'left-0.5'}" style="height: 18px; width: 18px;"></span>
      </button>
    </div>
  {/each}

  {#each TOOLS as tool (tool)}
    <div class={row}>
      <span class="text-[0.85rem] text-text-bright">{$_(`settings.${tool}`)}</span>
      <button class={openButton} disabled={$busy} on:click={() => open(tool)}>{$_("settings.open")}</button>
    </div>
  {/each}
</div>
