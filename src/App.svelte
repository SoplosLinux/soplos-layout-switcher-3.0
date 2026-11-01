<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { LogicalSize } from "@tauri-apps/api/dpi";
  import { getVersion } from "@tauri-apps/api/app";
  import { _, locale } from "svelte-i18n";
  import WindowControls from "./components/WindowControls.svelte";
  import { frameless, maximized } from "./lib/window";
  import { Settings as SettingsIcon, X, Sun, Moon } from "lucide-svelte";
  import { api } from "./lib/api";
  import { busy } from "./lib/jobs";
  import { theme, toggleTheme } from "./lib/theme";
  import { SUPPORTED_LOCALES, LOCALE_NAMES, setLocale, type SupportedLocale } from "./i18n";
  import type { EnvironmentInfo } from "./lib/layouts";
  import AboutDialog from "./components/AboutDialog.svelte";
  import DialogHost from "./components/DialogHost.svelte";
  import Layouts from "./views/Layouts.svelte";
  import Settings from "./views/Settings.svelte";

  // The frame of the other Soplos 3.0 programs without a sidebar (WebApp
  // Manager, AppImage Manager, Plymouth Manager): a hero with the title and the
  // language, a thin footer with the theme button, the session and the version,
  // and the progress bar along the bottom edge. What is inside is the layouts
  // of the 2.x program; its Settings page is a sidebar that stays hidden at the
  // left until the gear under the language is pressed. It stays loaded while
  // hidden.

  let appVersion = "";
  let environment: EnvironmentInfo = { desktopEnvironment: "unknown", displayProtocol: "unknown", shellVersion: "" };
  let aboutOpen = false;
  let settingsOpen = false;

  onMount(async () => {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = "";
    }
    try {
      environment = await api.invoke<EnvironmentInfo>("get_environment_info");
    } catch (error) {
      console.error("Reading the environment failed", error);
    }
  });

  // "3.0.0-1-beta" → "3.0.0-beta": the footer shows the app version, not the
  // Debian package revision.
  $: cleanVersion = appVersion.replace(/-\d+-/, "-");

  // Opening the settings makes the window wider by the width of the sidebar,
  // closing them gives it back, in a few short steps so it slides and does not
  // jump. While it moves, the layouts keep the width they had and the sidebar
  // grows beside them, anchored to its right edge, so it comes out from behind
  // them. The window is never moved, only resized: moving it as well made it
  // shake. The left edge stays, the window grows to the right and the layouts
  // go with it. A maximized or full screen window is not resized: the sidebar
  // slides in and takes room from the layouts.
  const SETTINGS_WIDTH = 300;
  const STEPS = 9;
  const STEP_MS = 16;

  let panelWidth = 0; // the width the sidebar has right now
  let mainWidth = 0; // the width the layouts are held at while it moves; 0 is free
  let smooth = false; // the sidebar animates by CSS, for a window that is not resized
  let moving = false;

  const ease = (t: number) => 1 - Math.pow(1 - t, 3);
  const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

  async function slide(win: ReturnType<typeof getCurrentWindow>, open: boolean) {
    smooth = false;
    const scale = await win.scaleFactor();
    const size = (await win.innerSize()).toLogical(scale);
    // Opening, the layouts are as wide as the window is; closing, as wide as it
    // is without the sidebar.
    const base = open ? size.width : size.width - SETTINGS_WIDTH;
    mainWidth = base;
    if (open) {
      panelWidth = 0;
      settingsOpen = true;
    }
    for (let step = 1; step <= STEPS; step++) {
      const grown = Math.round(SETTINGS_WIDTH * ease(step / STEPS));
      const width = open ? grown : SETTINGS_WIDTH - grown;
      panelWidth = width;
      await win.setSize(new LogicalSize(base + width, size.height));
      await pause(STEP_MS);
    }
    if (!open) settingsOpen = false;
  }

  async function setSettings(open: boolean) {
    if (moving || open === settingsOpen) return;
    moving = true;
    try {
      const win = getCurrentWindow();
      if ((await win.isMaximized()) || (await win.isFullscreen())) {
        smooth = true;
        if (open) {
          panelWidth = 0;
          settingsOpen = true;
          await pause(20);
          panelWidth = SETTINGS_WIDTH;
        } else {
          panelWidth = 0;
          await pause(220);
          settingsOpen = false;
        }
      } else {
        await slide(win, open);
      }
    } catch (error) {
      console.error("Moving the settings failed", error);
      settingsOpen = open;
      panelWidth = open ? SETTINGS_WIDTH : 0;
    }
    mainWidth = 0;
    moving = false;
  }

  function onLanguageChange(e: Event) {
    setLocale((e.currentTarget as HTMLSelectElement).value as SupportedLocale);
  }

  // F1 -> About, Ctrl+Q -> quit, Escape -> closes the settings.
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && settingsOpen) {
      setSettings(false);
    } else if (e.key === "F1") {
      e.preventDefault();
      aboutOpen = true;
    } else if (e.ctrlKey && !e.shiftKey && (e.key === "q" || e.key === "Q")) {
      e.preventDefault();
      getCurrentWindow().close();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="h-screen bg-dark {$frameless && !$maximized ? 'window-card' : ''}">
<div class="relative flex h-full overflow-hidden">
  <!-- The settings: a sidebar at the left of the layouts. Opening it makes the
       window wider by its own width, so it does not cover anything. -->
  {#if settingsOpen}
    <div class="flex shrink-0 justify-end overflow-hidden {smooth ? 'transition-[width] duration-200 ease-out' : ''}" style="width: {panelWidth}px">
      <aside class="flex w-[300px] shrink-0 flex-col border-r border-border bg-panel" aria-label={$_("tabs.settings")}>
        {#if $frameless}<div class="h-7 shrink-0" data-tauri-drag-region></div>{/if}
        <div class="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
          <h2 class="text-[0.9rem] font-semibold text-text-bright">{$_("tabs.settings")}</h2>
          <button class="rounded p-1 text-text-muted hover:bg-surface hover:text-text-bright" title={$_("common.close")} aria-label={$_("common.close")} on:click={() => setSettings(false)}>
            <X size={15} />
          </button>
        </div>
        <div class="flex-1 overflow-y-auto content-scroll p-4"><Settings /></div>
      </aside>
    </div>
  {/if}

  <div class="relative flex min-w-0 flex-1 flex-col" style={mainWidth ? `width: ${mainWidth}px; flex: none;` : ""}>
  {#if $frameless}
    <div class="relative h-7 shrink-0" data-tauri-drag-region>
      <div class="absolute right-2 top-[3px]"><WindowControls /></div>
    </div>
  {/if}

  <div class="p-4 pb-0 shrink-0 {$frameless ? 'pt-0' : ''}" data-tauri-drag-region="deep">
    <div
      class="relative rounded border border-border overflow-hidden flex flex-col items-start justify-end p-4 min-h-[110px]"
      style="background: linear-gradient(rgba(20, 20, 22, 0.25), rgba(20, 20, 22, 0.82)), url('/assets/hero/layout-switcher-background.jpg'), linear-gradient(135deg, #3a2412, #1b1b1d); background-size: cover; background-position: center;"
    >
      <div class="absolute top-3 right-3">
        <select
          value={$locale}
          on:change={onLanguageChange}
          class="appearance-none bg-primary/20 backdrop-blur-sm border border-primary/30 text-primary rounded pl-2 pr-7 py-1 text-[0.7rem] focus:outline-none"
          title={$_("sidebar.language")}
        >
          {#each SUPPORTED_LOCALES as code}
            <option value={code}>{LOCALE_NAMES[code]}</option>
          {/each}
        </select>
      </div>

      <button
        class="absolute bottom-3 right-3 grid h-6 w-6 place-items-center rounded border border-primary/30 bg-primary/20 text-primary backdrop-blur-sm hover:bg-primary/30"
        title={$_("tabs.settings")}
        aria-label={$_("tabs.settings")}
        aria-expanded={settingsOpen}
        on:click={() => setSettings(!settingsOpen)}
      >
        <SettingsIcon size={13} />
      </button>

      <p class="text-white font-semibold text-[0.95rem]">{$_("hero.title")}</p>
      <p class="text-[0.75rem] text-white/85 mt-0.5">{$_("hero.description")}</p>
    </div>
  </div>

  <div class="flex-1 overflow-y-auto content-scroll p-4">
    <Layouts />
  </div>

  <footer class="flex items-center justify-between px-4 py-1.5 border-t border-border text-[0.7rem] text-text-muted shrink-0">
    <div class="flex items-center gap-1.5">
      <button
        class="p-1 -m-1 rounded text-text-muted hover:bg-surface hover:text-text-bright"
        on:click={toggleTheme}
        title={$theme === "dark" ? $_("sidebar.switchToLight") : $_("sidebar.switchToDark")}
      >
        <svelte:component this={$theme === "dark" ? Moon : Sun} size={13} />
      </button>
      <span>
        {$_(`environment.${environment.desktopEnvironment}`)} · {$_(`environment.${environment.displayProtocol}`)}
      </span>
    </div>
    <button
      class="text-text-muted hover:text-text-bright hover:underline"
      on:click={() => (aboutOpen = true)}
      title={$_("sidebar.aboutTooltip")}
    >
      {cleanVersion}
    </button>
  </footer>

  <!-- The progress bar, as in the Welcome, the Store and the GRUB Editor: the
       last thing of the column, along the bottom edge of the window. Its track
       is only drawn while a job runs. -->
  <div class="h-1.5 bg-surface-2 overflow-hidden shrink-0" class:invisible={!$busy}>
    {#if $busy}
      <div class="h-full w-1/3 bg-primary progress-slide"></div>
    {/if}
  </div>
  </div>
</div>

{#if aboutOpen}
  <AboutDialog onClose={() => (aboutOpen = false)} />
{/if}

<DialogHost />
</div>
