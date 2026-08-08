/**
 * Address book — persistent list of saved recipient addresses, per network.
 *
 * Stored in localStorage (non-sensitive public metadata — addresses only, no
 * keys/seeds ever touch this). Used by the Send flow for quick "pick recipient".
 */

export interface AddressBookEntry {
  address: string;
  label: string;
  network: string;
  /** ISO timestamp of when it was added */
  addedAt: string;
}

const BOOK_KEY = 'foss_wallet_address_book';

export type AddressBook = AddressBookEntry[];

function loadBook(): AddressBook {
  try {
    const raw = localStorage.getItem(BOOK_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter(
      (e: unknown): e is AddressBookEntry =>
        !!e &&
        typeof (e as AddressBookEntry).address === 'string' &&
        typeof (e as AddressBookEntry).network === 'string'
    );
  } catch {
    return [];
  }
}

function saveBook(book: AddressBook): void {
  try {
    localStorage.setItem(BOOK_KEY, JSON.stringify(book));
  } catch {
    // localStorage unavailable — address book is best-effort
  }
}

/** All saved entries for a given network id. */
export function getAddressBook(network: string): AddressBook {
  return loadBook().filter((e) => e.network === network);
}

/** Look up a single saved entry by address (across all networks). */
export function findAddressEntry(address: string): AddressBookEntry | null {
  return loadBook().find((e) => e.address === address) ?? null;
}

/**
 * Add or update a recipient. If an entry with the same address already exists,
 * its label is updated (and network updated if the caller provides one).
 */
export function saveAddressEntry(address: string, label: string, network: string): void {
  const book = loadBook();
  const trimmedAddress = address.trim();
  if (!trimmedAddress) return;
  const existing = book.find((e) => e.address === trimmedAddress);
  if (existing) {
    existing.label = label.trim() || existing.label;
    existing.network = network || existing.network;
  } else {
    book.push({
      address: trimmedAddress,
      label: label.trim() || trimmedAddress,
      network,
      addedAt: new Date().toISOString(),
    });
  }
  saveBook(book);
}

/** Remove an entry by address. */
export function removeAddressEntry(address: string): void {
  const book = loadBook().filter((e) => e.address !== address);
  saveBook(book);
}

/** True if the given address is already saved. */
export function isAddressSaved(address: string): boolean {
  return loadBook().some((e) => e.address === address);
}

/** Total saved entries (for badge/UI counts). */
export function addressBookSize(): number {
  return loadBook().length;
}
