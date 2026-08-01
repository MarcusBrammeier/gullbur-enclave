#!/usr/bin/env node
/**
 * Standalone tests for shared UI utilities.
 * Uses Node built-in assert — no test framework dependency.
 */
import { describe, it, run } from 'node:test';
import assert from 'node:assert/strict';
import {
  truncateAddress,
  truncateAddressLong,
  truncateTxid,
  formatBalance,
  getBalanceFloat,
  getNetworkUnit,
  networkIcon,
  getNetworkBadge,
} from '../src/lib/utils.ts';

// ── Address Truncation ──────────────────────────────────────────────────────

await describe('truncateAddress', async () => {
  await it('returns short addresses as-is', () => {
    assert.equal(truncateAddress('abc'), 'abc');
    assert.equal(truncateAddress('123456789012'), '123456789012');
  });

  await it('truncates long addresses', () => {
    const addr = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq';
    assert.equal(truncateAddress(addr), 'bc1qar...5mdq');
    assert.equal(truncateAddress(addr).length, 13);
  });

  await it('handles empty input', () => {
    assert.equal(truncateAddress(''), '');
  });
});

await describe('truncateAddressLong', async () => {
  await it('truncates with longer tail', () => {
    const addr = 'bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq';
    assert.equal(truncateAddressLong(addr), 'bc1qar0s...wf5mdq');
  });
});

await describe('truncateTxid', async () => {
  await it('truncates long txids', () => {
    const txid = '0xabcdef1234567890abcdef1234567890abcdef1234567890';
    assert.ok(truncateTxid(txid).includes('...'));
  });
});

// ── Balance Formatting ──────────────────────────────────────────────────────

await describe('formatBalance', async () => {
  await it('formats a normal balance', () => {
    assert.equal(formatBalance({ confirmed: '1234.5678' }), '1,234.5678');
  });

  await it('returns 0 for null', () => {
    assert.equal(formatBalance(null), '0');
  });

  await it('returns 0 for NaN', () => {
    assert.equal(formatBalance({ confirmed: 'not-a-number' }), '0');
  });
});

await describe('getBalanceFloat', async () => {
  await it('parses balance string', () => {
    assert.equal(getBalanceFloat({ confirmed: '42.5' }), 42.5);
  });

  await it('returns 0 for null', () => {
    assert.equal(getBalanceFloat(null), 0);
  });
});

// ── Network Helpers ─────────────────────────────────────────────────────────

await describe('getNetworkUnit', async () => {
  await it('uses provided symbol', () => {
    assert.equal(getNetworkUnit('bitcoin', 'BTC'), 'BTC');
  });

  await it('falls back to uppercase', () => {
    assert.equal(getNetworkUnit('monero'), 'MONERO');
  });
});

await describe('networkIcon', async () => {
  await it('returns ₿ for bitcoin', () => {
    assert.equal(networkIcon('bitcoin'), '₿');
    assert.equal(networkIcon('bitcoin-testnet'), '₿');
  });

  await it('returns ◆ for unknown', () => {
    assert.equal(networkIcon('ethereum'), '◆');
  });
});

await describe('getNetworkBadge', async () => {
  await it('returns BTC badge', () => {
    assert.deepEqual(getNetworkBadge('bitcoin'), {
      label: 'BTC',
      color: 'bg-orange-600 text-orange-100',
    });
  });

  await it('handles unknown networks', () => {
    assert.deepEqual(getNetworkBadge('foobar'), {
      label: 'FOOBAR',
      color: 'bg-gray-600 text-gray-100',
    });
  });
});
