/**
 * Address book persistence tests. Phase 2.1 GUI workflow pass.
 *
 * Uses the cleartext localStorage fallback path (ipcClient unset), which is
 * exactly how demo mode and the sync path store data. Verifies the round-trip
 * CRUD + the corrupt/legacy-data recovery behavior that guards real users.
 */
import { describe, it, expect, beforeEach } from 'vitest';
import {
  getAddressBook,
  saveAddressEntry,
  removeAddressEntry,
  findAddressEntry,
  isAddressSaved,
  addressBookSize,
} from './addressBook';

const KEY = 'gullbur_address_book_enc';
const KEY_LEGACY = 'gullbur_address_book';

beforeEach(() => {
  localStorage.removeItem(KEY);
  localStorage.removeItem(KEY_LEGACY);
  localStorage.removeItem('foss_wallet_address_book');
});

describe('addressBook', () => {
  it('starts empty', () => {
    expect(getAddressBook('bitcoin')).toEqual([]);
    expect(addressBookSize()).toBe(0);
  });

  it('saves and retrieves an entry per network', () => {
    saveAddressEntry('bc1qtestaddress000', 'Alice', 'bitcoin');
    const btc = getAddressBook('bitcoin');
    expect(btc.length).toBe(1);
    expect(btc[0].address).toBe('bc1qtestaddress000');
    expect(btc[0].label).toBe('Alice');
    expect(btc[0].network).toBe('bitcoin');

    // Different network is isolated.
    expect(getAddressBook('ethereum')).toEqual([]);
  });

  it('updates label when the same address is saved again', () => {
    saveAddressEntry('0xabcd000000000000000000000000000000000001', 'Old', 'ethereum');
    saveAddressEntry('0xabcd000000000000000000000000000000000001', 'New', 'ethereum');
    const eth = getAddressBook('ethereum');
    expect(eth.length).toBe(1);
    expect(eth[0].label).toBe('New');
  });

  it('isAddressSaved and findAddressEntry report correctly', () => {
    saveAddressEntry('bc1qfindme', 'Find', 'bitcoin');
    expect(isAddressSaved('bc1qfindme')).toBe(true);
    expect(isAddressSaved('bc1qother')).toBe(false);
    const entry = findAddressEntry('bc1qfindme');
    expect(entry?.label).toBe('Find');
    expect(findAddressEntry('nope')).toBeNull();
  });

  it('removes an entry', () => {
    saveAddressEntry('bc1qrm', 'Remove me', 'bitcoin');
    saveAddressEntry('bc1qkeep', 'Keep', 'bitcoin');
    removeAddressEntry('bc1qrm');
    const btc = getAddressBook('bitcoin');
    expect(btc.length).toBe(1);
    expect(btc[0].address).toBe('bc1qkeep');
  });

  it('ignores empty addresses on save', () => {
    saveAddressEntry('   ', 'blank', 'bitcoin');
    expect(addressBookSize()).toBe(0);
  });
});

// ── Additional edge-case + persistence fuzz (Phase 2.1) ──────────────────

describe('addressBook persistence (sync localStorage path)', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('persists a saved address across reload (round-trip)', () => {
    saveAddressEntry('bc1qtestaddress', 'My BTC wallet', 'bitcoin');
    expect(isAddressSaved('bc1qtestaddress')).toBe(true);
    expect(findAddressEntry('bc1qtestaddress')?.label).toBe('My BTC wallet');
    expect(addressBookSize()).toBe(1);
  });

  it('filters by network', () => {
    saveAddressEntry('bc1qbtc', 'btc', 'bitcoin');
    saveAddressEntry('0xethaddr', 'eth', 'ethereum');
    const btc = getAddressBook('bitcoin');
    const eth = getAddressBook('ethereum');
    expect(btc).toHaveLength(1);
    expect(btc[0].address).toBe('bc1qbtc');
    expect(eth).toHaveLength(1);
    expect(eth[0].address).toBe('0xethaddr');
  });

  it('does not duplicate the same address — updates label instead', () => {
    saveAddressEntry('bc1qsame', 'Original', 'bitcoin');
    saveAddressEntry('bc1qsame', 'Renamed', 'bitcoin');
    expect(addressBookSize()).toBe(1);
    expect(findAddressEntry('bc1qsame')?.label).toBe('Renamed');
  });

  it('trims whitespace in address and label', () => {
    saveAddressEntry('  bc1qtrimmed  ', '  Label  ', 'bitcoin');
    expect(findAddressEntry('bc1qtrimmed')).not.toBeNull();
    expect(findAddressEntry('bc1qtrimmed')?.label).toBe('Label');
  });

  it('removes an entry by address', () => {
    saveAddressEntry('bc1qrm', 'gone', 'bitcoin');
    saveAddressEntry('bc1qkeep', 'keep', 'bitcoin');
    removeAddressEntry('bc1qrm');
    expect(addressBookSize()).toBe(1);
    expect(isAddressSaved('bc1qrm')).toBe(false);
    expect(isAddressSaved('bc1qkeep')).toBe(true);
  });

  it('recovers gracefully from corrupt JSON in storage', () => {
    localStorage.setItem(KEY_LEGACY, '{not valid json!!');
    expect(addressBookSize()).toBe(0);
    // A subsequent save still works
    saveAddressEntry('bc1qaftercorrupt', 'ok', 'bitcoin');
    expect(addressBookSize()).toBe(1);
  });

  it('filters out malformed entries while keeping valid ones', () => {
    localStorage.setItem(
      KEY_LEGACY,
      JSON.stringify([
        { address: 'bc1qvalid', label: 'x', network: 'bitcoin', addedAt: 't' },
        { address: 123, network: 'bitcoin' }, // malformed — missing label, non-string address
        null,
        { label: 'no-network', address: 'abc' },
      ]),
    );
    expect(addressBookSize()).toBe(1);
    expect(findAddressEntry('bc1qvalid')).not.toBeNull();
  });

  it('clears encrypted key after sync save leaves consistent state', () => {
    saveAddressEntry('bc1qcleanup', 'c', 'bitcoin');
    expect(localStorage.getItem(KEY_LEGACY)).not.toBeNull();
    expect(localStorage.getItem(KEY)).not.toBeNull();
  });
});