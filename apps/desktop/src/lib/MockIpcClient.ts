/**
 * MockIpcClient — simulates vault-core IPC for the demo UI.
 * Returns realistic fake data for every vault method.
  */

 interface PendingCall {
   resolve: (v: unknown) => void;
   reject: (e: Error) => void;
 }

 export class MockIpcClient {
  private nextId = 1;
  private pending = new Map<number, PendingCall>();
  private connected = false;
  onAuthRequired?: () => void;

  async connect(): Promise<void> {
    this.connected = true;
    // Simulate a brief connection delay
    await new Promise(r => setTimeout(r, 500));
  }

  disconnect(): void {
    this.connected = false;
    for (const [, p] of this.pending) {
      p.reject(new Error('Connection closed'));
    }
    this.pending.clear();
  }

  async call(method: string, params: unknown): Promise<unknown> {
    const id = this.nextId++;
    if (!this.connected) throw new Error('Not connected');

    // Simulate network delay
    await new Promise(r => setTimeout(r, 200 + Math.random() * 300));

    return this.handleMethod(method, params, id);
  }

  private handleMethod(method: string, params: unknown, _id: number): unknown {
    switch (method) {
      case 'vault.status':
        return {
          initialized: true,
          connected: true,
          status: 'Connected',
          tor_enabled: false,
          active_plugins: ['btc', 'evm', 'xmr'],
          networks: [
            { id: 'ethereum', name: 'Ethereum', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
            { id: 'arbitrum', name: 'Arbitrum One', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
            { id: 'base', name: 'Base', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
            { id: 'polygon', name: 'Polygon', symbol: 'POL', decimals: 18, is_testnet: false, active: true, unit: 'POL' },
            { id: 'sepolia', name: 'Sepolia', symbol: 'ETH', decimals: 18, is_testnet: true, active: true, unit: 'ETH' },
          ],
          accounts: [
            { id: 'ethereum-0', network: 'ethereum', address: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045', balance: { confirmed: '3.452', unconfirmed: '0' }, index: 0, path: "m/44'/60'/0'/0/0" },
            { id: 'ethereum-1', network: 'ethereum', address: '0x71C7656EC7ab88b098defB751B7401B5f6d8976F', balance: { confirmed: '0.001', unconfirmed: '0' }, index: 1, path: "m/44'/60'/0'/0/1" },
            { id: 'polygon-0', network: 'polygon', address: '0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97', balance: { confirmed: '12.8', unconfirmed: '0' }, index: 0, path: "m/44'/60'/0'/0/0" },
            { id: 'sepolia-0', network: 'sepolia', address: '0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B', balance: { confirmed: '0.5', unconfirmed: '0.01' }, index: 0, path: "m/44'/60'/0'/0/0" },
          ],
        };

      case 'vault.initialize':
        return { success: true, mnemonic: 'abandon ability able about above absent absorb abstract absurd abuse access accident account accuse achieve acid acoustic acquire across act action actor actress actual' };

      case 'vault.create_account':
        return {
          account: {
            id: `${(params as any).network}-${(params as any).index ?? 0}`,
            network: (params as any).network,
            address: '0x' + Array.from({ length: 40 }, () => Math.floor(Math.random() * 16).toString(16)).join(''),
            balance: { confirmed: '0', unconfirmed: '0' },
            index: (params as any).index ?? 0,
            path: `m/44'/60'/${(params as any).index ?? 0}'/0/0`,
          },
        };

      case 'vault.get_balance':
        return { balance: (Math.random() * 10).toFixed(3) };

      case 'vault.sign_transaction':
        return { signed_tx: '0x02f86b010285...' + Array.from({ length: 20 }, () => Math.floor(Math.random() * 16).toString(16)).join('') };

      case 'vault.broadcast_transaction':
        return { tx_hash: '0x' + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join('') };

      case 'vault.get_transaction_history':
        return {
          transactions: [
            {
              txid: '0x' + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join(''),
              from: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
              to: '0x71C7656EC7ab88b098defB751B7401B5f6d8976F',
              amount: '0.05',
              unit: 'ETH',
              direction: 'sent',
              status: 'confirmed',
              blockHeight: 11234567,
              timestamp: Math.floor(Date.now() / 1000 - 3600),
            },
            {
              txid: '0x' + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join(''),
              from: '0x71C7656EC7ab88b098defB751B7401B5f6d8976F',
              to: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
              amount: '0.12',
              unit: 'ETH',
              direction: 'received',
              status: 'confirmed',
              blockHeight: 11234500,
              timestamp: Math.floor(Date.now() / 1000 - 7200),
            },
            {
              txid: '0x' + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join(''),
              from: '0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045',
              to: '0x4838B106FCe9647Bdf1E7877BF73cE8B0BAD5f97',
              amount: '1.0',
              unit: 'ETH',
              direction: 'sent',
              status: 'confirmed',
              blockHeight: 11234400,
              timestamp: Math.floor(Date.now() / 1000 - 14400),
            },
            {
              txid: '0x' + Array.from({ length: 64 }, () => Math.floor(Math.random() * 16).toString(16)).join(''),
              from: '',
              to: '0xAb5801a7D398351b8bE11C439e05C5B3259aeC9B',
              amount: '0.5',
              unit: 'ETH',
              direction: 'received',
              status: 'pending',
              blockHeight: undefined,
              timestamp: Math.floor(Date.now() / 1000 - 300),
            },
          ],
        };

      case 'vault.estimate_fee':
        return { fast: '15.2', medium: '12.8', slow: '10.1', unit: 'gwei' };

      case 'vault.validate_address':
        return true;

      case 'vault.list_networks':
        return {
          networks: [
            { id: 'ethereum', name: 'Ethereum', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
            { id: 'arbitrum', name: 'Arbitrum One', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
            { id: 'base', name: 'Base', symbol: 'ETH', decimals: 18, is_testnet: false, active: true, unit: 'ETH' },
            { id: 'polygon', name: 'Polygon', symbol: 'POL', decimals: 18, is_testnet: false, active: true, unit: 'POL' },
            { id: 'sepolia', name: 'Sepolia', symbol: 'ETH', decimals: 18, is_testnet: true, active: true, unit: 'ETH' },
          ],
        };

      case 'vault.generate_mnemonic':
        return { mnemonic: 'abandon ability able about above absent absorb abstract absurd abuse access accident account accuse achieve acid acoustic acquire across act action actor actress actual' };

      case 'lock_vault':
      case 'vault.lock':
        return { success: true };

      case 'confirm_hardware':
        return { success: true };

      case 'get_security_stats':
        return { auth_status: 'biometric_unlocked', auto_lock_remaining: 28, failed_attempts: 0, biometry_enabled: true };

      case 'toggle_tor':
        return { tor_enabled: (params as any)?.enabled ?? true };

      default:
        throw new Error(`Unknown method: ${method}`);
    }
  }
}