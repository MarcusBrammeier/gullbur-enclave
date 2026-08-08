/**
 * Address book store tests.
 */
import { describe, it, expect, beforeEach } from 'vitest';
import {
  getAddressBook,
  saveAddressEntry,
  removeAddressEntry,
  isAddressSaved,
  findAddressEntry,
  addressBookSize,
} from './addressBook';

const KEY = 'foss_wallet_address_book';

beforeEach(() => {
  localStorage.removeItem(KEY);
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
