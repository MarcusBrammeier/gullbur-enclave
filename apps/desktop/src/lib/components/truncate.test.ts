/**
 * Test to verify truncateTxid format
 */
import { describe, it, expect } from 'vitest';
import { truncateTxid } from '../utils';

describe('truncateTxid', () => {
  it('truncates long txid correctly', () => {
    const result = truncateTxid('0xaaa111bbb222ccccdddd');
    console.log('Result:', result);
    console.log('Length:', result.length);
    // Format: "0xaaa111...ccdddd" (first 8 chars + "..." + last 6 chars)
    expect(result).toBe('0xaaa111...ccdddd');
  });

  it('truncates another txid', () => {
    const result = truncateTxid('0xddd333eee444ffffgggg');
    expect(result).toBe('0xddd333...ffgggg');
  });

  it('truncates third txid', () => {
    const result = truncateTxid('0xggg555hhh666iiiijjjj');
    expect(result).toBe('0xggg555...iijjjj');
  });
});