/**
 * Source group loading utilities.
 *
 * Documentation: [docs/device-discovery.md](../../../docs/device-discovery.md)
 */

export interface IncrementalLoadOptions<TKey, TItem> {
  keys: TKey[];
  buildLoadingItem: (key: TKey) => TItem;
  fetchItem: (key: TKey) => Promise<TItem>;
  getItemKey: (item: TItem) => string;
  sortItems: (items: TItem[]) => TItem[];
  onUpdate: (items: TItem[]) => void;
  isActive?: () => boolean;
}

export function upsertSortedItem<TItem>(
  items: TItem[],
  nextItem: TItem,
  getItemKey: (item: TItem) => string,
  sortItems: (items: TItem[]) => TItem[],
): TItem[] {
  const nextItems = items.filter(
    (item) => getItemKey(item) !== getItemKey(nextItem),
  );
  nextItems.push(nextItem);
  return sortItems(nextItems);
}

export async function loadItemsIncrementally<TKey, TItem>(
  options: IncrementalLoadOptions<TKey, TItem>,
): Promise<void> {
  const {
    keys,
    buildLoadingItem,
    fetchItem,
    getItemKey,
    sortItems,
    onUpdate,
    isActive,
  } = options;

  let items = sortItems(keys.map((key) => buildLoadingItem(key)));
  onUpdate(items);

  await Promise.allSettled(
    keys.map(async (key) => {
      const nextItem = await fetchItem(key);
      if (isActive && !isActive()) {
        return;
      }

      items = upsertSortedItem(items, nextItem, getItemKey, sortItems);
      onUpdate(items);
    }),
  );
}
