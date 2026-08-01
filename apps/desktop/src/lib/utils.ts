/**
 * Shared UI utilities for Gullbúr Enclave Core.
 *
 * Centralises functions duplicated across multiple components to
 * keep formatting, truncation, and badge logic consistent.
 */
import type { Balance, NetworkSpec } from './types';

// ── Address Truncation ──────────────────────────────────────────────────────

/** Truncate an address for compact display: `6ab...cd34` */
export function truncateAddress(addr: string): string {
  if (!addr || addr.length <= 12) return addr ?? '';
  return `${addr.slice(0, 6)}...${addr.slice(-4)}`;
}

/** Truncate an address with a longer tail for address-detail contexts */
export function truncateAddressLong(addr: string): string {
  if (!addr || addr.length <= 16) return addr ?? '';
  return `${addr.slice(0, 8)}...${addr.slice(-6)}`;
}

/** Truncate a txid for compact display */
export function truncateTxid(txid: string): string {
  if (!txid || txid.length <= 16) return txid ?? '';
  return `${txid.slice(0, 8)}...${txid.slice(-6)}`;
}

// ── Balance Formatting ──────────────────────────────────────────────────────

/** Format a balance value for display with up to 8 decimal places */
export function formatBalance(balance: Balance | null): string {
  if (!balance) return '0';
  const val = parseFloat(balance.confirmed);
  if (isNaN(val)) return '0';
  return val.toLocaleString(undefined, { maximumFractionDigits: 8 });
}

/** Parse a balance to its raw numeric value */
export function getBalanceFloat(balance: Balance | null): number {
  if (!balance) return 0;
  const val = parseFloat(balance.confirmed);
  return isNaN(val) ? 0 : val;
}

// ── Network Helpers ─────────────────────────────────────────────────────────

/** Look up a network spec by its ID */
export function getNetworkSpec(networks: NetworkSpec[], id: string): NetworkSpec | undefined {
  return networks.find((n) => n.id === id);
}

/** Unit string for a network (e.g. BTC, ETH, XMR) */
export function getNetworkUnit(networkId: string, symbol?: string): string {
  return symbol ?? networkId.toUpperCase();
}

/** Icon character for a network */
export function networkIcon(networkId: string): string {
  if (networkId.includes('bitcoin')) return '₿';
  if (networkId.includes('monero')) return 'ɱ';
  if (networkId.includes('litecoin')) return 'Ł';
  return '◆';
}

// ── Network Badge ───────────────────────────────────────────────────────────

export interface BadgeInfo {
  label: string;
  color: string;
}

/**
 * Returns a label + Tailwind colour classes for a network badge.
 * Used in Dashboard, Portfolio, and account listing contexts.
 */
export function getNetworkBadge(networkId: string): BadgeInfo {
  switch (networkId) {
    case 'bitcoin':
    case 'bitcoin-testnet':
      return { label: 'BTC', color: 'bg-orange-600 text-orange-100' };
    case 'litecoin':
    case 'litecoin-testnet':
      return { label: 'LTC', color: 'bg-gray-400 text-gray-900' };
    case 'monero':
    case 'monero-stagenet':
      return { label: 'XMR', color: 'bg-orange-500 text-orange-100' };
    case 'ethereum':
    case 'sepolia':
      return { label: 'ETH', color: 'bg-blue-600 text-blue-100' };
    case 'polygon':
      return { label: 'POL', color: 'bg-purple-600 text-purple-100' };
    case 'arbitrum':
      return { label: 'ARB', color: 'bg-sky-600 text-sky-100' };
    case 'base':
      return { label: 'BASE', color: 'bg-blue-500 text-blue-100' };
    case 'optimism':
      return { label: 'OP', color: 'bg-red-500 text-red-100' };
    case 'bnb':
      return { label: 'BNB', color: 'bg-yellow-500 text-yellow-100' };
    default:
      return { label: networkId.toUpperCase(), color: 'bg-gray-600 text-gray-100' };
  }
}
