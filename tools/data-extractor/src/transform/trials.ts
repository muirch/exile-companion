// Reshapes `LabyrinthTrials` into poebo's flat `TrialInfo` schema: the campaign Trials of
// Ascendancy and where to find them. Each trial references a WorldArea (exported as a row index);
// we resolve it to the area's id / name / act and keep only campaign areas (acts 1-10, structured
// campaign id), dropping the endgame (Uber Lab, act 11) trials.

import type { RawWorldArea } from './areas.js';

/** A `LabyrinthTrials` row: a trial located in one WorldArea (a row index into WorldAreas). */
export interface RawLabyrinthTrial {
  _index: number;
  WorldAreas: number | null;
}

/** One campaign lab trial location (mirrors `poebo_core::data::TrialInfo`). */
export interface TrialInfo {
  /** WorldArea id the trial is in, e.g. `1_1_7_1` (The Lower Prison). */
  area_id: string;
  area_name: string;
  act: number;
}

// Same campaign-id pattern as areas: `<part>_<act>_<n>`. Endgame trials have ids like
// `EndGame_Labyrinth_trials_spikes` and act 11, so this cleanly drops them.
const CAMPAIGN_ID = /^[12]_\d+_/;

export function transformTrials(
  trials: RawLabyrinthTrial[],
  worldAreas: RawWorldArea[],
): TrialInfo[] {
  const areaByIndex = new Map(worldAreas.map((w) => [w._index, w]));
  const seen = new Set<string>();
  const out: TrialInfo[] = [];

  for (const t of trials) {
    if (t.WorldAreas == null) continue;
    const area = areaByIndex.get(t.WorldAreas);
    if (!area || !area.Id || !CAMPAIGN_ID.test(area.Id) || area.Act < 1 || area.Act > 10) continue;
    if (seen.has(area.Id)) continue;
    seen.add(area.Id);
    out.push({ area_id: area.Id, area_name: area.Name, act: area.Act });
  }

  out.sort((a, b) => a.act - b.act || a.area_id.localeCompare(b.area_id));
  return out;
}
