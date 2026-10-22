// Reshapes raw `WorldAreas` rows (as exported by pathofexile-dat) into Exile Companion's flat `ZoneInfo`
// schema. Connectivity is exported as row *indices* into WorldAreas; we resolve those to `Id`s and
// keep only connections that stay within the emitted campaign set, so the pack is self-contained.

/** A `WorldAreas` row as emitted by pathofexile-dat (only the columns we request). */
export interface RawWorldArea {
  _index: number;
  Id: string;
  Name: string;
  Act: number;
  AreaLevel: number;
  IsTown: boolean;
  IsHideout: boolean;
  IsMapArea: boolean;
  HasWaypoint: boolean;
  Connections_WorldAreasKeys: number[];
}

/** One campaign area in the data pack (mirrors `exile_core::data::ZoneInfo`). */
export interface ZoneInfo {
  id: string;
  name: string;
  act: number;
  area_level: number;
  is_town: boolean;
  is_hideout: boolean;
  is_map: boolean;
  has_waypoint: boolean;
  connections: string[];
}

// Campaign main-zone ids look like `<part>_<act>_<n>` - e.g. `1_1_1`, `1_1_town`, `2_6_1`. This
// pattern cleanly excludes the ~280 non-campaign areas that also carry `Act` in 1..10 (Character
// Select, hideouts, and league mechanics like Betrayal/Abyss/Descent whose ids are non-numeric).
const CAMPAIGN_ID = /^[12]_\d+_/;

/** Campaign main zones = acts 1..10, not a map (Atlas) area, with a structured campaign id. */
function isCampaignArea(r: RawWorldArea): boolean {
  return (
    Boolean(r.Id) &&
    Boolean(r.Name) &&
    r.Act >= 1 &&
    r.Act <= 10 &&
    !r.IsMapArea &&
    CAMPAIGN_ID.test(r.Id)
  );
}

export function transformAreas(rows: RawWorldArea[]): ZoneInfo[] {
  const indexToId = new Map<number, string>();
  for (const r of rows) {
    if (r.Id) indexToId.set(r._index, r.Id);
  }

  const campaign = rows.filter(isCampaignArea);
  const campaignIds = new Set(campaign.map((r) => r.Id));

  const areas: ZoneInfo[] = campaign.map((r) => ({
    id: r.Id,
    name: r.Name,
    act: r.Act,
    area_level: r.AreaLevel,
    is_town: r.IsTown,
    is_hideout: r.IsHideout,
    is_map: r.IsMapArea,
    has_waypoint: r.HasWaypoint,
    connections: (r.Connections_WorldAreasKeys ?? [])
      .map((i) => indexToId.get(i))
      .filter((id): id is string => Boolean(id) && campaignIds.has(id as string))
      // de-duplicate (a few areas list the same connection twice)
      .filter((id, i, arr) => arr.indexOf(id) === i),
  }));

  // Stable, human-reviewable ordering so per-patch git diffs are readable.
  areas.sort((a, b) => a.act - b.act || a.area_level - b.area_level || a.id.localeCompare(b.id));
  return areas;
}
