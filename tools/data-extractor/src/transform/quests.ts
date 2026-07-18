// Reshapes the quest / quest-reward tables (as exported by pathofexile-dat) into poebo's flat
// `QuestInfo` schema: one entry per campaign quest, carrying the gem rewards it grants keyed by the
// classes that can claim them.
//
// The join is: QuestRewards.RewardOffer -> QuestRewardOffers.QuestKey -> Quest (id/act/name), plus
// QuestRewards.Reward -> BaseItemTypes (gem id/name) and QuestRewards.Characters -> Characters
// (class names). Foreign keys export as row *indices* into the referenced table, same as the
// WorldAreas connections; we resolve them here. Only gem rewards are kept (Reward whose BaseItemType
// id is under `Metadata/Items/Gems/`); other reward items (flasks, currency) are dropped.

/** A `Quest` row (only the columns we request). */
export interface RawQuest {
  _index: number;
  Id: string;
  Act: number;
  Name: string;
}

/** A `QuestRewards` row: one reward item for one offer, restricted to a set of classes. */
export interface RawQuestReward {
  _index: number;
  RewardOffer: number | null;
  Characters: number[];
  Reward: number | null;
  RewardLevel: number;
}

/** A `QuestRewardOffers` row: links a reward offer to its quest. */
export interface RawQuestRewardOffer {
  _index: number;
  Id: string;
  QuestKey: number | null;
}

/** A `Characters` row: one playable class. `Name` is the display class name (e.g. "Witch"). */
export interface RawCharacter {
  _index: number;
  Id: string;
  Name: string;
}

/** A `BaseItemTypes` row. `Id` is the metadata path (matches Path of Building's gemId). */
export interface RawBaseItemType {
  _index: number;
  Id: string;
  Name: string;
}

/** One gem a quest can reward, and the classes eligible to claim it. */
export interface GemReward {
  /** Metadata path, e.g. `Metadata/Items/Gems/SkillGemFireball` - the join key to a PoB gem. */
  gem_id: string;
  /** Display name, e.g. "Fireball". */
  gem_name: string;
  /** The gem level the reward grants. */
  reward_level: number;
  /** Class names that can claim this reward (e.g. ["Witch", "Templar"]). */
  classes: string[];
}

/** One campaign quest and its gem rewards (mirrors `poebo_core::data::QuestInfo`). */
export interface QuestInfo {
  /** Quest id, e.g. `a1q1`. */
  id: string;
  act: number;
  name: string;
  gem_rewards: GemReward[];
}

// Main-campaign quests are ided `a<act>q<n>` (e.g. `a1q1`, `a10q3`). This excludes league/tutorial
// quests (synthesis, hellscape, MercenariesTutorial, ...) that also carry an Act.
const CAMPAIGN_QUEST = /^a([1-9]|10)q\d+$/;
const GEM_ID = /^Metadata\/Items\/Gems\//;

export function transformQuests(
  quests: RawQuest[],
  rewards: RawQuestReward[],
  offers: RawQuestRewardOffer[],
  characters: RawCharacter[],
  baseItems: RawBaseItemType[],
): QuestInfo[] {
  const questByIndex = new Map(quests.map((q) => [q._index, q]));
  const offerToQuest = new Map(offers.map((o) => [o._index, o.QuestKey]));
  const className = new Map(characters.map((c) => [c._index, c.Name]));
  const itemByIndex = new Map(baseItems.map((b) => [b._index, b]));

  // Accumulate per quest, de-duplicating a gem across class-specific reward rows (the same gem is
  // often listed once per class), unioning the eligible classes.
  const acc = new Map<string, { info: QuestInfo; gems: Map<string, GemReward> }>();

  for (const r of rewards) {
    if (r.RewardOffer == null || r.Reward == null) continue;
    const questIndex = offerToQuest.get(r.RewardOffer);
    if (questIndex == null) continue;
    const quest = questByIndex.get(questIndex);
    if (!quest || !CAMPAIGN_QUEST.test(quest.Id)) continue;
    const item = itemByIndex.get(r.Reward);
    if (!item || !item.Id || !GEM_ID.test(item.Id)) continue;

    let entry = acc.get(quest.Id);
    if (!entry) {
      entry = {
        info: { id: quest.Id, act: quest.Act, name: quest.Name, gem_rewards: [] },
        gems: new Map(),
      };
      acc.set(quest.Id, entry);
    }

    const classes = (r.Characters ?? [])
      .map((ci) => className.get(ci))
      .filter((n): n is string => Boolean(n));

    const existing = entry.gems.get(item.Id);
    if (existing) {
      for (const c of classes) if (!existing.classes.includes(c)) existing.classes.push(c);
    } else {
      entry.gems.set(item.Id, {
        gem_id: item.Id,
        gem_name: item.Name,
        reward_level: r.RewardLevel,
        classes: [...classes],
      });
    }
  }

  const out: QuestInfo[] = [];
  for (const { info, gems } of acc.values()) {
    info.gem_rewards = [...gems.values()].sort((a, b) => a.gem_name.localeCompare(b.gem_name));
    for (const g of info.gem_rewards) g.classes.sort();
    out.push(info);
  }
  // Stable, human-reviewable ordering so per-patch git diffs are readable.
  out.sort((a, b) => a.act - b.act || a.id.localeCompare(b.id));
  return out;
}
