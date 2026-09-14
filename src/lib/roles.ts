// Riot's live-client-data and LCU position strings, in the conventional
// Top/Jungle/Mid/ADC/Support lane order — used to sort team lists instead
// of leaving them in whatever (effectively random) order the client APIs
// report players in.
const ROLE_ORDER = ["TOP", "JUNGLE", "MIDDLE", "BOTTOM", "UTILITY"];

function roleRank(position: string): number {
  const idx = ROLE_ORDER.indexOf(position.toUpperCase());
  return idx === -1 ? ROLE_ORDER.length : idx;
}

export function sortByPosition<T>(items: T[], getPosition: (item: T) => string): T[] {
  return [...items].sort((a, b) => roleRank(getPosition(a)) - roleRank(getPosition(b)));
}
