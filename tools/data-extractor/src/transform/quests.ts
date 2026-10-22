// Reshapes the quest / quest-reward tables (as exported by pathofexile-dat) into Exile Companion's flat
// `QuestInfo` schema: one entry per campaign quest, carrying the gem rewards it grants keyed by the
// classes that can claim them, plus the NPC you claim them from.
//
// The join is: QuestRewards.RewardOffer -> QuestRewardOffers.QuestKey -> Quest (id/act/name), plus
// QuestRewards.Reward -> BaseItemTypes (gem id/name) and QuestRewards.Characters -> Characters
// (class names). The reward NPC comes from NPCTalk.QuestRewardOffersKey -> the same offer, resolving
// NPCTalk.NPCKey -> NPCs.Name. Foreign keys export as row *indices* into the referenced table, same
// as the WorldAreas connections; we resolve them here.
//
// Two standard rewards are kept per quest: gem rewards (Reward whose BaseItemType id is under
// `Metadata/Items/Gems/`) and passive skill points (a "Book of Skill" quest item = +1 point). Other
// reward items (keys, flasks, "Book of Regrets" respec) are dropped. Reward offers whose id carries a
// `_tencent` suffix are region-specific (the Chinese/Tencent client) and are excluded - so a quest
// that only "gives a gem" through a Tencent offer correctly shows no gem on the standard client
// (e.g. An Indomitable Spirit gives a Book of Skill, not a gem).

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

/** An `NPCTalk` row: a dialogue that may present a reward offer, spoken by an NPC. */
export interface RawNPCTalk {
  _index: number;
  NPCKey: number | null;
  QuestRewardOffersKey: number | null;
}

/** An `NPCs` row. `Name` is the NPC's display name (e.g. "Tarkleigh", "Nessa"). */
export interface RawNPC {
  _index: number;
  Id: string;
  Name: string;
}

// Service/placeholder NPC names that show up in reward dialogue but aren't the quest giver.
const NON_NPC_NAMES = new Set(['', 'Tutorial NPC']);

// A few reward offers are presented via a tutorial/service dialogue, so the NPCTalk join yields no
// real NPC. Hand-filled from the game (verified by quest name), applied only as a fallback.
const NPC_OVERRIDES: Record<string, string> = {
  a4q6: 'Tasuni', // An Indomitable Spirit (Book of Skill)
};

// The Book-of-Skill quest item grants one passive skill point; "Book of Regrets" is a respec, not a
// point, so it is excluded.
const PASSIVE_POINT_ITEM = 'Book of Skill';

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

/** One campaign quest with its standard rewards (mirrors `exile_core::data::QuestInfo`). */
export interface QuestInfo {
  /** Quest id, e.g. `a1q1`. */
  id: string;
  act: number;
  name: string;
  /** The NPC you claim this quest's reward from (e.g. "Tarkleigh"); null if unresolved. */
  npc: string | null;
  /** Passive skill points this quest grants (Book of Skill count); usually 0 or 1. */
  passive_points: number;
  gem_rewards: GemReward[];
}

// Main-campaign quests are ided `a<act>q<n>`, sometimes with a letter suffix (`a1q1`, `a10q3`, `a5q1b`). This excludes league/tutorial
// quests (synthesis, hellscape, MercenariesTutorial, ...) that also carry an Act.
const CAMPAIGN_QUEST = /^a([1-9]|10)q\d+[a-z]?$/;
const GEM_ID = /^Metadata\/Items\/Gems\//;

export function transformQuests(
  quests: RawQuest[],
  rewards: RawQuestReward[],
  offers: RawQuestRewardOffer[],
  characters: RawCharacter[],
  baseItems: RawBaseItemType[],
  npcTalk: RawNPCTalk[],
  npcs: RawNPC[],
): QuestInfo[] {
  const questByIndex = new Map(quests.map((q) => [q._index, q]));
  const offerToQuest = new Map(offers.map((o) => [o._index, o.QuestKey]));
  const className = new Map(characters.map((c) => [c._index, c.Name]));
  const itemByIndex = new Map(baseItems.map((b) => [b._index, b]));
  const npcName = new Map(npcs.map((n) => [n._index, n.Name]));
  // Region-specific (Tencent/Chinese client) reward offers - excluded from the standard pack.
  const tencentOffers = new Set(
    offers.filter((o) => o.Id.toLowerCase().includes('tencent')).map((o) => o._index),
  );

  // Reward offer -> the NPC most often quoted presenting it (skipping placeholder/tutorial names).
  const offerNpcNames = new Map<number, string[]>();
  for (const t of npcTalk) {
    if (t.QuestRewardOffersKey == null) continue;
    const name = t.NPCKey == null ? undefined : npcName.get(t.NPCKey);
    if (!name || NON_NPC_NAMES.has(name)) continue;
    const list = offerNpcNames.get(t.QuestRewardOffersKey) ?? [];
    list.push(name);
    offerNpcNames.set(t.QuestRewardOffersKey, list);
  }
  const offerNpc = (offerIndex: number): string | null => {
    const names = offerNpcNames.get(offerIndex);
    if (!names || names.length === 0) return null;
    const counts = new Map<string, number>();
    for (const n of names) counts.set(n, (counts.get(n) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => b[1] - a[1])[0][0];
  };

  // Every campaign quest is emitted: routes reference quests that give no gem or passive point.
  const acc = new Map<string, { info: QuestInfo; gems: Map<string, GemReward> }>();

  const ensure = (quest: RawQuest, offerIndex: number) => {
    let entry = acc.get(quest.Id);
    if (!entry) {
      entry = {
        info: {
          id: quest.Id,
          act: quest.Act,
          name: quest.Name,
          npc: offerNpc(offerIndex) ?? NPC_OVERRIDES[quest.Id] ?? null,
          passive_points: 0,
          gem_rewards: [],
        },
        gems: new Map(),
      };
      acc.set(quest.Id, entry);
    } else if (entry.info.npc == null) {
      // Fill the NPC from a later offer if the first one had none.
      entry.info.npc = offerNpc(offerIndex) ?? NPC_OVERRIDES[quest.Id] ?? null;
    }
    return entry;
  };

  for (const q of quests) {
    if (CAMPAIGN_QUEST.test(q.Id)) ensure(q, -1);
  }

  for (const r of rewards) {
    if (r.RewardOffer == null || r.Reward == null) continue;
    if (tencentOffers.has(r.RewardOffer)) continue;
    const questIndex = offerToQuest.get(r.RewardOffer);
    if (questIndex == null) continue;
    const quest = questByIndex.get(questIndex);
    if (!quest || !CAMPAIGN_QUEST.test(quest.Id)) continue;
    const item = itemByIndex.get(r.Reward);
    if (!item || !item.Id) continue;

    if (GEM_ID.test(item.Id)) {
      const entry = ensure(quest, r.RewardOffer);
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
    } else if (item.Name === PASSIVE_POINT_ITEM) {
      ensure(quest, r.RewardOffer).info.passive_points += 1;
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
