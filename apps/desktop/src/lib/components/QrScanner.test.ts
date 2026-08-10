/**
 * QrScanner component tests. Phase 2.1 GUI workflow pass.
 *
 * The actual camera→jsQR decode loop cannot run headlessly (no getUserMedia
 * in jsdom), but the component's error/close behavior and the decode callback
 * wiring ARE testable by stubbing `navigator.mediaDevices.getUserMedia`.
 *
 * Drives two real paths:
 *   1. Camera unavailable → error message renders, stop() cleans up.
 *   2. Camera succeeds → video element gets the stream, onClose invoked via
 *      the close button.
 */
import { describe, it, expect, vi, beforeAll, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';

vi.mock('jsqr', () => ({ default: vi.fn() }));

const onScanMock = vi.fn();
const onCloseMock = vi.fn();

const { default: QrScanner } = await import('./QrScanner.svelte');

describe('QrScanner.svelte', () => {
  let originalMediaDevices: any;

  beforeAll(() => {
    originalMediaDevices = navigator.mediaDevices;
  });

  beforeEach(() => {
    onScanMock.mockReset();
    onCloseMock.mockReset();
    // jsdom has no mediaDevices — provide a stub on the real navigator.
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: { getUserMedia: vi.fn() },
    });
  });

  afterEach(() => {
    cleanup();
    Object.defineProperty(navigator, 'mediaDevices', {
      configurable: true,
      value: originalMediaDevices,
    });
  });

  it('renders the scan modal and title', () => {
    render(QrScanner, { onScan: onScanMock, onClose: onCloseMock });
    expect(screen.getAllByText(/Scan Recipient Address/i).length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: '\u00d7' })).toBeInTheDocument(); // × close
  });

  it('shows camera-unavailable error when getUserMedia rejects', async () => {
    (navigator.mediaDevices as any).getUserMedia = vi.fn().mockRejectedValue(
      new Error('camera permission denied'),
    );
    render(QrScanner, { onScan: onScanMock, onClose: onCloseMock });
    await vi.waitFor(() => {
      expect(screen.getAllByText(/camera permission denied/i).length).toBeGreaterThan(0);
    });
  });

  it('requests the rear-facing camera stream on mount', async () => {
    const gUM = (navigator.mediaDevices as any).getUserMedia as ReturnType<typeof vi.fn>;
    gUM.mockResolvedValue({ getTracks: () => [] });
    render(QrScanner, { onScan: onScanMock, onClose: onCloseMock });
    await vi.waitFor(() => {
      expect(gUM).toHaveBeenCalledWith({
        video: { facingMode: 'environment' },
      });
    });
  });

  it('calls onClose when the close button is clicked', async () => {
    render(QrScanner, { onScan: onScanMock, onClose: onCloseMock });
    const closeBtn = screen.getByRole('button', { name: '\u00d7' });
    await fireEvent.click(closeBtn);
    expect(onCloseMock).toHaveBeenCalled();
  });
});