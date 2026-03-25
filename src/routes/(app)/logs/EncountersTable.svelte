<script lang="ts">
  import QuickTooltip from "$lib/components/QuickTooltip.svelte";
  import { raidGates } from "$lib/constants/encounters";
  import { IconStar } from "$lib/icons";
  import type { EncounterPreview, EncountersOverview } from "$lib/types";
  import type { SvelteSet } from "svelte/reactivity";
  import {
    abbreviateNumber,
    formatTimestamp,
    getClassIcon,
    isSupportSpec,
    timestampToMinutesAndSeconds
  } from "$lib/utils";
  import { encounterFilter, settings, type sortColumns } from "$lib/stores.svelte";
  import type { EncounterGroup } from "./groupEncounters";

  let {
    overview,
    selectMode,
    selected = $bindable(),
    groups = [],
    collapsed = false
  }: {
    overview: EncountersOverview;
    selectMode: boolean;
    selected: SvelteSet<number>;
    groups?: EncounterGroup[];
    collapsed?: boolean;
  } = $props();

  let allSelected = $derived(overview.encounters.every((enc) => selected.has(enc.id)));
  let expandedGroups = $state(new Set<number>());

  function changeSort(sort: sortColumns) {
    encounterFilter.sort = sort;
    encounterFilter.order = encounterFilter.order === "asc" ? "desc" : "asc";
  }

  function toggleGroup(repId: number) {
    if (expandedGroups.has(repId)) {
      expandedGroups.delete(repId);
    } else {
      expandedGroups.add(repId);
    }
    expandedGroups = new Set(expandedGroups);
  }

  const buffColors = ["text-red-300", "text-green-300", "text-yellow-300", "text-blue-300"];
</script>

{#snippet checkbox(id: number)}
  <input
    type="checkbox"
    checked={selected.has(id)}
    onchange={() => {
      selected.has(id) ? selected.delete(id) : selected.add(id);
    }}
    class="form-checkbox checked:text-accent-600/80 size-5 rounded-sm border-0 bg-neutral-700 focus:ring-0"
  />
{/snippet}
{#snippet encounterRow(encounter: EncounterPreview, indent?: boolean)}
  {@const gate = raidGates[encounter.bossName]}
  {@const isSupport = isSupportSpec(encounter.spec)}
  {@const buffs = [encounter.supportAp, encounter.supportBrand, encounter.supportIdentity, encounter.supportHyper]}
  <tr class="items-center border-b border-neutral-700/50 hover:bg-neutral-800 {indent ? 'border-l-2 border-l-accent-500/50' : ''}">
    <!-- Encounter ID -->
    <td class="text-center">
      {#if indent}
        <div class="p-2 pl-4 text-neutral-500" class:text-lime-400={encounter.cleared}>
          #{encounter.id}
        </div>
      {:else if !selectMode}
        <div class="p-2" class:text-lime-400={encounter.cleared}>
          #{encounter.id}
        </div>
      {:else}
        {@render checkbox(encounter.id)}
      {/if}
    </td>
    <!-- Encounter info: Difficulty, Gate, Name, Favorite -->
    <td class="w-full py-2 pl-3 pr-1 font-medium">
      <div class="flex flex-col gap-1">
        <div class="flex gap-1 text-nowrap text-neutral-300">
          {#if encounter.difficulty}
            <p
              class="py-.5 rounded-sm bg-neutral-700/80 px-1 text-xs"
              class:text-yellow-300={encounter.difficulty === "Hard"}
              class:text-amber-600={encounter.difficulty === "Inferno" ||
                encounter.difficulty === "Challenge" ||
                encounter.difficulty === "Trial"}
              class:text-cyan-400={encounter.difficulty === "Solo"}
              class:text-purple-500={encounter.difficulty.includes("Extreme") || encounter.difficulty === "The First"}
            >
              {encounter.difficulty}
            </p>
          {/if}
          {#if gate}
            <p class="py-.5 truncate rounded-sm bg-neutral-700/80 px-1 text-xs">
              {gate}
            </p>
          {/if}
          {#if settings.app.general.experimentalFeatures && !encounter.cleared && encounter.wipeBars}
            <p class="py-.5 rounded-sm bg-neutral-700/80 px-1 text-xs text-red-300">
              Wipe - {encounter.wipeBars}x
            </p>
          {/if}
        </div>
        <a
          href="/logs/{encounter.id}"
          class="hover:text-accent-500 group flex items-center gap-1 text-sm hover:underline"
        >
          {#if encounter.favorite}
            <IconStar class="shrink-0 text-yellow-400" />
          {/if}
          <QuickTooltip tooltip={encounter.bossName || "No Boss"} class="truncate">
            {encounter.bossName || "No Boss"}
          </QuickTooltip>
        </a>
      </div>
    </td>
    <!-- Classes -->
    <td class="p-3">
      <div class="mask-r-from-80% mask-r-to-100% flex">
        {#each encounter.classes as classId, i}
          <QuickTooltip tooltip={encounter.names[i]} class="shrink-0">
            <img src={getClassIcon(classId)} alt="class-{classId}" class="size-8" />
          </QuickTooltip>
        {/each}
      </div>
    </td>
    <!-- Local Player Name -->
    <td class="p-1">
      <div class="flex">
        <QuickTooltip tooltip={encounter.localPlayer} class="truncate">
          {encounter.localPlayer}
        </QuickTooltip>
      </div>
    </td>
    <!-- Performance (DPS / Buffs) -->
    <td class="hidden p-1 text-right md:table-cell">
      {#if isSupport && buffs.some((b) => b)}
        <QuickTooltip tooltip="AP · Brand · Identity · T">
          <div class="flex items-center justify-end gap-0.5">
            {#each buffs as buff, i}
              <span class="text-sm {buffColors[i]}">
                {(buff! * 100).toFixed(0)}
              </span>
              {#if i < buffs.length - 1}
                <span class="text-neutral-400">·</span>
              {/if}
            {/each}
          </div>
        </QuickTooltip>
      {:else}
        {abbreviateNumber(encounter.myDps)}
      {/if}
    </td>
    <td class="hidden p-1 text-right lg:table-cell">
      {#if encounter.udps && encounter.udps !== encounter.myDps}
        {abbreviateNumber(encounter.udps)}
      {:else}
        -
      {/if}
    </td>
    <!-- Duration -->
    <td class="p-1 text-right">
      {timestampToMinutesAndSeconds(encounter.duration)}
    </td>
    <!-- Date -->
    <td class="pr-2 text-right text-xs">
      {formatTimestamp(encounter.fightStart)}
    </td>
  </tr>
{/snippet}

{#snippet groupHeader(group: EncounterGroup)}
  {@const rep = group.representative}
  {@const gate = raidGates[rep.bossName]}
  {@const isExpanded = expandedGroups.has(rep.id)}
  {@const attemptCount = group.attempts.length}
  {@const isSupport = isSupportSpec(rep.spec)}
  {@const buffs = [rep.supportAp, rep.supportBrand, rep.supportIdentity, rep.supportHyper]}
  <tr class="items-center border-b border-neutral-700/50 hover:bg-neutral-800">
    <!-- Encounter ID -->
    <td class="text-center">
      {#if !selectMode}
        <div class="p-2" class:text-lime-400={rep.cleared}>
          #{rep.id}
        </div>
      {:else}
        {@render checkbox(rep.id)}
      {/if}
    </td>
    <!-- Encounter info -->
    <td class="w-full py-2 pl-3 pr-1 font-medium">
      <div class="flex flex-col gap-1">
        <div class="flex items-center gap-1 text-nowrap text-neutral-300">
          {#if attemptCount > 1}
            <button
              class="rounded-sm bg-neutral-700 px-1.5 py-0.5 text-xs font-medium hover:bg-neutral-600 {isExpanded ? 'text-accent-400' : 'text-neutral-300'}"
              onclick={() => toggleGroup(rep.id)}
            >
              {isExpanded ? "▾" : "▸"}
            </button>
          {/if}
          {#if rep.difficulty}
            <p
              class="py-.5 rounded-sm bg-neutral-700/80 px-1 text-xs"
              class:text-yellow-300={rep.difficulty === "Hard"}
              class:text-amber-600={rep.difficulty === "Inferno" ||
                rep.difficulty === "Challenge" ||
                rep.difficulty === "Trial"}
              class:text-cyan-400={rep.difficulty === "Solo"}
              class:text-purple-500={rep.difficulty.includes("Extreme") || rep.difficulty === "The First"}
            >
              {rep.difficulty}
            </p>
          {/if}
          {#if gate}
            <p class="py-.5 truncate rounded-sm bg-neutral-700/80 px-1 text-xs">
              {gate}
            </p>
          {/if}
          {#if settings.app.general.experimentalFeatures && !rep.cleared && rep.wipeBars}
            <p class="py-.5 rounded-sm bg-neutral-700/80 px-1 text-xs text-red-300">
              Wipe - {rep.wipeBars}x
            </p>
          {/if}
          {#if attemptCount > 1}
            <a
              href="/logs/progression?ids={group.attempts.map((a) => a.id).join(',')}"
              class="py-.5 rounded-sm bg-accent-500/30 px-1 text-xs text-accent-300 hover:bg-accent-500/50"
            >
              {attemptCount} attempts
            </a>
          {/if}
        </div>
        <a
          href="/logs/{rep.id}"
          class="hover:text-accent-500 group flex items-center gap-1 text-sm hover:underline"
        >
          {#if rep.favorite}
            <IconStar class="shrink-0 text-yellow-400" />
          {/if}
          <QuickTooltip tooltip={rep.bossName || "No Boss"} class="truncate">
            {rep.bossName || "No Boss"}
          </QuickTooltip>
        </a>
      </div>
    </td>
    <!-- Classes -->
    <td class="p-3">
      <div class="mask-r-from-80% mask-r-to-100% flex">
        {#each rep.classes as classId, i}
          <QuickTooltip tooltip={rep.names[i]} class="shrink-0">
            <img src={getClassIcon(classId)} alt="class-{classId}" class="size-8" />
          </QuickTooltip>
        {/each}
      </div>
    </td>
    <!-- Local Player Name -->
    <td class="p-1">
      <div class="flex">
        <QuickTooltip tooltip={rep.localPlayer} class="truncate">
          {rep.localPlayer}
        </QuickTooltip>
      </div>
    </td>
    <!-- Performance -->
    <td class="hidden p-1 text-right md:table-cell">
      {#if isSupport && buffs.some((b) => b)}
        <QuickTooltip tooltip="AP · Brand · Identity · T">
          <div class="flex items-center justify-end gap-0.5">
            {#each buffs as buff, i}
              <span class="text-sm {buffColors[i]}">
                {(buff! * 100).toFixed(0)}
              </span>
              {#if i < buffs.length - 1}
                <span class="text-neutral-400">·</span>
              {/if}
            {/each}
          </div>
        </QuickTooltip>
      {:else}
        {abbreviateNumber(rep.myDps)}
      {/if}
    </td>
    <td class="hidden p-1 text-right lg:table-cell">
      {#if rep.udps && rep.udps !== rep.myDps}
        {abbreviateNumber(rep.udps)}
      {:else}
        -
      {/if}
    </td>
    <!-- Duration -->
    <td class="p-1 text-right">
      {timestampToMinutesAndSeconds(rep.duration)}
    </td>
    <!-- Date -->
    <td class="pr-2 text-right text-xs">
      {formatTimestamp(rep.fightStart)}
    </td>
  </tr>
  {#if isExpanded}
    {#each group.attempts as attempt (attempt.id)}
      {#if attempt.id !== rep.id}
        {@render encounterRow(attempt, true)}
      {/if}
    {/each}
  {/if}
{/snippet}

<table data-table="encounters" class="w-full table-fixed">
  <thead class="sticky top-0 z-10 bg-[#121212]/95 shadow-lg backdrop-blur-lg">
    <tr>
      {#if !selectMode}
        <th
          class="w-14 cursor-pointer p-3 {encounterFilter.sort === 'id' && encounterFilter.order === 'asc'
            ? 'text-accent-500/80'
            : 'hover:opacity-80'}"
          onclick={() => changeSort("id")}
        >
          ID
        </th>
      {:else}
        <th class="w-14 p-3">
          <input
            type="checkbox"
            checked={allSelected}
            onchange={() => {
              if (allSelected) {
                for (const enc of overview.encounters) {
                  selected.delete(enc.id);
                }
              } else {
                for (const enc of overview.encounters) {
                  selected.add(enc.id);
                }
              }
            }}
            class="form-checkbox checked:text-accent-600/80 size-4.5 rounded-sm border-0 bg-neutral-700 focus:ring-0"
          />
        </th>
      {/if}
      <th class="w-[25%] p-3 text-left">Encounter</th>
      <th class="p-3 text-left">Classes</th>
      <th class="w-24 px-1 text-left lg:w-32">Name</th>
      <th
        class="hidden w-28 cursor-pointer px-1 text-right md:table-cell {encounterFilter.sort === 'my_dps'
          ? 'text-accent-500/80'
          : 'hover:opacity-80'}"
        onclick={() => changeSort("my_dps")}>Performance</th
      >
      <th
        class="hidden w-20 px-1 text-right lg:table-cell {encounterFilter.sort === 'unbuffed_dps'
          ? 'text-accent-500/80'
          : 'hover:opacity-80'}"
        onclick={() => changeSort("unbuffed_dps")}>uDPS</th
      >
      <th
        class="w-24 cursor-pointer px-1 text-right {encounterFilter.sort === 'duration'
          ? 'text-accent-500/80'
          : 'hover:opacity-80'}"
        onclick={() => changeSort("duration")}>Duration</th
      >
      <th class="w-24 pr-2 text-right xl:w-36">Date</th>
    </tr>
  </thead>
  <tbody class="text-neutral-200">
    {#if collapsed && groups.length > 0}
      {#each groups as group (group.representative.id)}
        {#if group.attempts.length === 1}
          {@render encounterRow(group.representative)}
        {:else}
          {@render groupHeader(group)}
        {/if}
      {/each}
    {:else}
      {#each overview.encounters as enc}
        {@render encounterRow(enc)}
      {/each}
    {/if}
  </tbody>
</table>
