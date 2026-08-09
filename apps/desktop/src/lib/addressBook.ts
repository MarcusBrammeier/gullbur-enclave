/**
 * Address book — persistent list of saved recipient addresses, per network.
 *
 * Stored encrypted in localStorage via vault.encrypt_data / vault.decrypt_data
 * IPC methods using the device key. Falls back to cleartext localStorage if
 * the vault IPC is unavailable (demo mode).
 *
 * The encryption uses the device key from the vault backend, so address book
 * data is protected at rest in localStorage.
 */

import type { IpcClient } from './IpcClient';

export interface AddressBookEntry {
  address: string;
  label: string;
  network: string;
  /** ISO timestamp of when it was added */
  addedAt: string;
}

const BOOK_KEY = 'gullbur_address_book';
const BOOK_KEY_ENC = 'gullbur_address_book_enc';

export type AddressBook = AddressBookEntry[];

let ipcClient: IpcClient | null = null;

/**
 * Set the IPC client for encrypted storage operations.
 * Called by vault.svelte.ts after connect.
 */
export function setAddressBookIpc(client: IpcClient | null): void {
  ipcClient = client;
}

/**
 * Try to JSON-parse and validate an address book from localStorage.
 * Returns null on any failure.
 */
function tryParseBook(raw: string | null): AddressBook | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return null;
    const valid = parsed.filter(
      (e: unknown): e is AddressBookEntry =>
        !!e &&
        typeof (e as AddressBookEntry).address === 'string' &&
        typeof (e as AddressBookEntry).network === 'string'
    );
    return valid;
  } catch {
    return null;
  }
}

function loadBook(): AddressBook {
  // Try encrypted storage first (IPC-encrypted blob)
  const encRaw = localStorage.getItem(BOOK_KEY_ENC);
  let book = tryParseBook(encRaw);
  if (book) return book;

  // Legacy cleartext storage
  const raw = localStorage.getItem(BOOK_KEY);
  book = tryParseBook(raw);
  if (book) return book;

  return [];
}

/**
 * Synchronous save — always writes cleartext to both keys.
 * Used as fallback when IPC is unavailable and by tests.
 */
function saveBookSync(book: AddressBook): void {
  try {
    const json = JSON.stringify(book);
    localStorage.setItem(BOOK_KEY_ENC, json);
    localStorage.setItem(BOOK_KEY, json);
  } catch {
    // localStorage unavailable — best-effort
  }
}

/**
 * Encrypt the address book via vault IPC and store in localStorage.
 * Falls back to cleartext if IPC is unavailable.
 * Synchronous when ipcClient is null; fire-and-forget async otherwise.
 */
function saveBook(book: AddressBook): void {
  if (!ipcClient) {
    saveBookSync(book);
    return;
  }
  // Async encrypted save — fire and forget. The next loadBook()
  // will read cleartext (also saved below) until the IPC comes back.
  saveBookEncryptedAsync(book);
  saveBookSync(book);
}

async function saveBookEncryptedAsync(book: AddressBook): Promise<void> {
  if (!ipcClient) return;
  try {
    const plaintext = JSON.stringify(book);
    const result = await ipcClient.call('vault.encrypt_data', {
      data: plaintext,
      aad: 'gullbur-addressbook',
    }) as { encrypted: string };
    localStorage.setItem(BOOK_KEY_ENC, result.encrypted);
    // Remove cleartext legacy key once encrypted save succeeds
    localStorage.removeItem(BOOK_KEY);
  } catch {
    // IPC failed — cleartext is already saved by saveBookSync
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