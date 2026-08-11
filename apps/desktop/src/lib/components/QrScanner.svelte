<script lang="ts">
  /**
   * QrScanner — captures a QR code from the device camera and decodes it.
   *
   * Uses jsQR (pure-TS, no build script) to decode a QR from a getUserMedia
   * video stream. On a successful decode it calls `onScan(address)` then keeps
   * the camera open until the user closes (repeated/false reads are de-duped).
   */
  import jsQR from 'jsqr';
  import { iconHtml } from '../icons';

  interface Props {
    onScan: (text: string) => void;
    onClose: () => void;
  }

  let { onScan, onClose }: Props = $props();

  let videoRef = $state<HTMLVideoElement | null>(null);
  let canvas = $state<HTMLCanvasElement | null>(null);
  let stream: MediaStream | null = null;
  let scanning = $state(false);
  let errorMsg = $state('');
  let lastScanned = $state('');

  async function startCamera() {
    errorMsg = '';
    scanning = true;
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { facingMode: 'environment' },
      });
      if (videoRef) {
        videoRef.srcObject = stream;
        await videoRef.play();
        requestAnimationFrame(tick);
      }
    } catch (e) {
      scanning = false;
      errorMsg = e instanceof Error ? e.message : 'Camera unavailable';
    }
  }

  function tick() {
    if (!videoRef || !videoRef.videoWidth) {
      if (scanning) requestAnimationFrame(tick);
      return;
    }
    try {
      if (!canvas) {
        canvas = document.createElement('canvas');
        document.body.appendChild(canvas);
      }
      canvas.width = videoRef.videoWidth;
      canvas.height = videoRef.videoHeight;
      const ctx = canvas.getContext('2d', { willReadFrequently: true });
      if (!ctx) { if (scanning) requestAnimationFrame(tick); return; }
      ctx.drawImage(videoRef, 0, 0, canvas.width, canvas.height);
      const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
      const code = jsQR(imageData.data, imageData.width, imageData.height);
      if (code?.data && code.data !== lastScanned) {
        lastScanned = code.data;
        onScan(code.data.trim());
        return; // stop scanning loop after one successful capture
      }
    } catch { /* ignore decode errors, keep scanning */ }
    if (scanning) requestAnimationFrame(tick);
  }

  function stop() {
    scanning = false;
    stream?.getTracks().forEach((t) => t.stop());
    stream = null;
    if (canvas) { canvas.remove(); canvas = null; }
  }

  function close() {
    stop();
    onClose();
  }

  $effect(() => {
    startCamera();
    return stop;
  });
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4">
  <div class="bg-surface-dim border border-strong rounded-2xl shadow-2xl max-w-md w-full p-6">
    <div class="flex items-center justify-between mb-4">
      <h2 class="text-lg font-semibold">{@html iconHtml('camera', 'w-5 h-5 inline-block mr-2')}Scan Recipient Address</h2>
      <button class="text-muted hover:text-primary text-xl leading-none" onclick={close}>&times;</button>
    </div>

    <div class="relative bg-black rounded-xl overflow-hidden aspect-square">
      <video bind:this={videoRef} class="w-full h-full object-cover" autoplay muted playsinline></video>
      {#if scanning && !videoRef?.videoWidth}
        <div class="absolute inset-0 flex items-center justify-center text-sm text-muted">Accessing camera…</div>
      {/if}
      {#if errorMsg}
        <div class="absolute inset-0 flex items-center justify-center p-6 text-center text-sm text-red-400">{errorMsg}</div>
      {/if}
    </div>

    <p class="text-xs text-secondary mt-3 text-center">
      Point your camera at a wallet-address QR code.
    </p>
    {#if lastScanned}
      <p class="text-xs text-vault-400 mt-1 text-center">{@html iconHtml('check', 'w-4 h-4 inline-block mr-1')}Found: <span class="font-mono break-all">{lastScanned}</span></p>
    {/if}
  </div>
</div>
