import { raidGates } from "$lib/constants/encounters";
import type { EncounterPreview } from "$lib/types";

export interface EncounterGroup {
  representative: EncounterPreview;
  attempts: EncounterPreview[];
}

function getGateKey(encounter: EncounterPreview): string {
  const gate = raidGates[encounter.bossName] || encounter.bossName;
  const difficulty = encounter.difficulty || "";
  return `${gate}|${difficulty}`;
}

/**
 * Returns true if `names` is a subset of `roster`.
 * Used to handle short fights where not every player registers in the log.
 */
function isSubsetOf(names: Set<string>, roster: Set<string>): boolean {
  for (const name of names) {
    if (!roster.has(name)) return false;
  }
  return true;
}

/**
 * Groups consecutive encounters that share the same gate, difficulty, and party composition.
 * Short fights where only a subset of party members registered are still grouped together.
 * The representative is the cleared encounter if one exists, otherwise the first (most recent).
 */
export function groupEncounters(encounters: EncounterPreview[]): EncounterGroup[] {
  if (encounters.length === 0) return [];

  const groups: EncounterGroup[] = [];
  let currentAttempts: EncounterPreview[] = [encounters[0]];
  let currentGateKey = getGateKey(encounters[0]);
  let currentRoster = new Set(encounters[0].names);

  for (let i = 1; i < encounters.length; i++) {
    const enc = encounters[i];
    const gateKey = getGateKey(enc);
    const encNames = new Set(enc.names);

    if (
      gateKey === currentGateKey &&
      (isSubsetOf(encNames, currentRoster) || isSubsetOf(currentRoster, encNames))
    ) {
      currentAttempts.push(enc);
      // Expand roster if this encounter has more players (it's the "full" group)
      for (const name of encNames) {
        currentRoster.add(name);
      }
    } else {
      groups.push(buildGroup(currentAttempts));
      currentAttempts = [enc];
      currentGateKey = gateKey;
      currentRoster = encNames;
    }
  }

  groups.push(buildGroup(currentAttempts));
  return groups;
}

function buildGroup(attempts: EncounterPreview[]): EncounterGroup {
  const cleared = attempts.find((a) => a.cleared);
  return {
    representative: cleared || attempts[0],
    attempts
  };
}

/**
 * Returns how many raw encounters were consumed to form complete groups,
 * dropping the last group if it might be incomplete (split at page boundary).
 * If all encounters form one group, keep it as-is.
 */
export function trimIncompleteGroups(
  groups: EncounterGroup[],
  requestedPageSize: number,
  totalFetched: number
): { groups: EncounterGroup[]; consumedCount: number } {
  if (groups.length <= 1) {
    const count = groups.reduce((sum, g) => sum + g.attempts.length, 0);
    return { groups, consumedCount: count };
  }

  // If we fetched fewer than requested, we have all the data - no trimming needed
  if (totalFetched <= requestedPageSize) {
    const count = groups.reduce((sum, g) => sum + g.attempts.length, 0);
    return { groups, consumedCount: count };
  }

  // The last group might be split - drop it
  const trimmed = groups.slice(0, -1);
  const count = trimmed.reduce((sum, g) => sum + g.attempts.length, 0);
  return { groups: trimmed, consumedCount: count };
}
