/**
 * Tests for shared UI utilities.
 */
import { describe, it, expect } from 'vitest';
import {
  truncateAddress,
  truncateAddressLong,
  truncateTxid,
  formatBalance,
  getBalanceFloat,
  getNetworkUnit,
  networkIcon,
  getNetworkBadge,
} from './utils';

// ── Address Truncation ──────────────────────────────────────────────────────

describe('truncateAddress', () => {
  it('returns short addresses as-is', () => {
    expect(truncateAddress('abc')).toBe('abc');
    expect(truncateAddress('123456789012')).toBe('123456789012');
  });

  it('truncates long addresses', () => {
    const addr = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq';
    expect(truncateAddress(addr)).toBe('bc1qar...5mdq');
    expect(truncateAddress(addr).length).toBe(13);
  });

  it('handles empty input gracefully', () => {
    expect(truncateAddress('')).toBe('');
  });
});

describe('truncateAddressLong', () => {
  it('returns short addresses as-is', () => {
    expect(truncateAddressLong('abc')).toBe('abc');
  });

  it('truncates with longer tail', () => {
    const addr = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq';
    expect(truncateAddressLong(addr)).toBe('bc1qar0s...wf5mdq');
  });
});

describe('truncateTxid', () => {
  it('truncates long txids', () => {
    const txid = '0xabcdef1234567890abcdef1234567890abcdef1234567890';
    expect(truncateTxid(txid)).toContain('...');
  });

  it('returns short txids as-is', () => {
    expect(truncateTxid('shorttxid')).toBe('shorttxid');
  });
});

// ── Balance Formatting ──────────────────────────────────────────────────────

describe('formatBalance', () => {
  it('formats a normal balance', () => {
    expect(formatBalance({ confirmed: '1234.5678' })).toBe('1,234.5678');
  });

  it('returns 0 for null balance', () => {
    expect(formatBalance(null)).toBe('0');
  });

  it('returns 0 for NaN balance', () => {
    expect(formatBalance({ confirmed: 'not-a-number' })).toBe('0');
  });

  it('handles zero', () => {
    expect(formatBalance({ confirmed: '0' })).toBe('0');
  });
});

describe('getBalanceFloat', () => {
  it('parses a balance string', () => {
    expect(getBalanceFloat({ confirmed: '42.5' })).toBe(42.5);
  });

  it('returns 0 for null', () => {
    expect(getBalanceFloat(null)).toBe(0);
  });
});

// ── Network Helpers ─────────────────────────────────────────────────────────

describe('getNetworkUnit', () => {
  it('uses provided symbol over uppercase ID', () => {
    expect(getNetworkUnit('bitcoin', 'BTC')).toBe('BTC');
  });

  it('falls back to uppercase ID', () => {
    expect(getNetworkUnit('monero')).toBe('MONERO');
  });
});

describe('networkIcon', () => {
  it('returns ₿ for bitcoin', () => {
    expect(networkIcon('bitcoin')).toBe('₿');
    expect(networkIcon('bitcoin-testnet')).toBe('₿');
  });

  it('returns ɱ for monero', () => {
    expect(networkIcon('monero')).toBe('ɱ');
  });

  it('returns Ł for litecoin', () => {
    expect(networkIcon('litecoin')).toBe('Ł');
  });

  it('returns ◆ for unknown', () => {
    expect(networkIcon('ethereum')).toBe('◆');
  });
});

// ── Network Badge ───────────────────────────────────────────────────────────

describe('getNetworkBadge', () => {
  it('returns BTC badge with orange bg', () => {
    expect(getNetworkBadge('bitcoin')).toEqual({ label: 'BTC', color: 'bg-orange-600 text-orange-100' });
  });

  it('returns ETH badge with blue bg', () => {
    expect(getNetworkBadge('ethereum')).toEqual({ label: 'ETH', color: 'bg-blue-600 text-blue-100' });
  });

  it('returns XMR badge with orange bg', () => {
    expect(getNetworkBadge('monero')).toEqual({ label: 'XMR', color: 'bg-orange-500 text-orange-100' });
  });

  it('returns LTC badge with gray bg', () => {
    expect(getNetworkBadge('litecoin')).toEqual({ label: 'LTC', color: 'bg-gray-400 text-gray-900' });
  });

  it('handles unknown networks', () => {
    expect(getNetworkBadge('foobar')).toEqual({ label: 'FOOBAR', color: 'bg-gray-600 text-gray-100' });
  });
});
