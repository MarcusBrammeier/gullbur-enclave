/**
 * JSON-RPC response shapes for vault IPC methods.
 * Mirrors the Rust backend types exposed over the WebSocket bridge.
 */

/** A supported blockchain network specification */
export interface NetworkSpec {
  id: string;
  name: string;
  symbol: string;
  decimals: number;
  /** Display unit (e.g. BTC, ETH, XMR) */
  unit?: string;
  /** Whether the network is active and available */
  active: boolean;
  /** Whether this is a testnet */
  is_testnet?: boolean;
}

/** Balance info for a single account */
export interface Balance {
  confirmed: string;
  unconfirmed?: string;
}

/** A wallet account derived from the seed phrase */
export interface Account {
  id: string;
  network: string;
  address: string;
  index: number;
  balance: Balance | null;
  /** BIP44 derivation path, e.g. "m/84'/0'/0'/0/0" */
  path?: string;
  /** Display label for the account */
  label?: string;
}

/** Fee estimate from the vault */
export interface FeeEstimate {
  level: 'fast' | 'medium' | 'slow';
  fee: number | string;
  estimatedTime?: string;
}

/** Response from vault.status */
export interface VaultStatusResponse {
  initialized: boolean;
  networks: NetworkSpec[];
  accounts: Account[];
  /** Optional human-readable status string */
  status?: string;
}

/** Response from vault.initialize */
export interface VaultInitializeResponse {
  success: boolean;
  /** Optional mnemonic phrase returned after generation */
  mnemonic?: string;
}

/** Response from vault.get_balance */
export interface VaultGetBalanceResponse {
  network: string;
  address: string;
  balance: Balance;
}

/** Response from vault.list_networks */
export interface VaultListNetworksResponse {
  networks: NetworkSpec[];
}

/** Response from vault.generate_mnemonic (if supported) */
export interface VaultGenerateMnemonicResponse {
  mnemonic: string;
}

/** A transaction record (for history display) */
export interface TxRecord {
  txid: string;
  from: string;
  to: string;
  amount: string;
  unit: string;
  direction: 'sent' | 'received';
  status: 'pending' | 'confirmed' | 'failed';
  timestamp?: number;
  blockHeight?: number;
}

/** A key handle reference */
export interface KeyHandle {
  id: string;
  network: string;
  algorithm: string;
}

/** Response from vault.validate_address */
export interface VaultValidateAddressResponse {
  valid: boolean;
}

/** Response from vault.estimate_fee */
export interface VaultEstimateFeeResponse {
  estimates: FeeEstimate[];
}

/** Response from vault.sign_transaction */
export interface VaultSignTransactionResponse {
  signed_tx: string;
  txid?: string;
}

/** Response from vault.simulate_transfer */
export interface VaultSimulateTransferResponse {
  success: boolean;
  gasUsed: number;
  gasEstimate: string;
  returnData: string;
  revertReason: string | null;
}

/** Response from vault.broadcast_transaction */
export interface VaultBroadcastTransactionResponse {
  txid: string;
}

/** Response from vault.get_transaction_history */
export interface VaultGetTransactionHistoryResponse {
  transactions: TxRecord[];
}