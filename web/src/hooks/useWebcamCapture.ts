'use client';

/**
 * useWebcamCapture — task #481 step 5
 *
 * React hook that manages a webcam stream, captures snapshots, and applies
 * image preprocessing before returning a base64 data URL.
 *
 * Preprocessing pipeline (runs on an offscreen canvas):
 *   1. Crop to center square  (min dimension)
 *   2. Desaturate             (luminance formula)
 *   3. +20% contrast          (pixel deviation from 128 × 1.2)
 *
 * All MediaDevices calls are wrapped in try/catch and surface errors via the
 * `error` field rather than throwing.
 */

import { useRef, useState, useCallback } from 'react';

// ---------------------------------------------------------------------------
// Public return shape
// ---------------------------------------------------------------------------

export interface UseWebcamCaptureReturn {
  /** Attach to the <video> element that should receive the camera stream. */
  videoRef: React.RefObject<HTMLVideoElement | null>;
  /** True once the camera stream is active. */
  isStreaming: boolean;
  /** True after the user grants camera permission (or when a stream exists). */
  hasPermission: boolean;
  /** Human-readable error string, or null when everything is fine. */
  error: string | null;
  /** Start the camera stream, requesting 1280×720 HD with a {video: true} fallback. */
  startCamera: () => Promise<void>;
  /** Stop all active tracks and revoke any object URLs. */
  stopCamera: () => void;
  /**
   * Capture the current video frame, apply preprocessing, and return the
   * resulting JPEG data URL.  Also updates `previewUrl`.
   * Returns null if the stream is not active.
   */
  captureSnapshot: () => Promise<string | null>;
  /** Data URL of the last captured (preprocessed) snapshot, or null. */
  previewUrl: string | null;
  /** True while the canvas preprocessing is running. */
  isProcessing: boolean;
}

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useWebcamCapture(): UseWebcamCaptureReturn {
  const videoRef = useRef<HTMLVideoElement | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const previewObjectUrlRef = useRef<string | null>(null);

  const [isStreaming, setIsStreaming] = useState(false);
  const [hasPermission, setHasPermission] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [previewUrl, setPreviewUrl] = useState<string | null>(null);
  const [isProcessing, setIsProcessing] = useState(false);

  // --------------------------------------------------------------------------
  // startCamera
  // --------------------------------------------------------------------------

  const startCamera = useCallback(async () => {
    setError(null);

    if (typeof window === 'undefined' || !navigator?.mediaDevices?.getUserMedia) {
      setError('Camera API is not available in this browser.');
      return;
    }

    let stream: MediaStream | null = null;

    // Attempt HD first, fall back to whatever the browser offers
    try {
      stream = await navigator.mediaDevices.getUserMedia({
        video: { width: { ideal: 1280 }, height: { ideal: 720 } },
      });
    } catch {
      try {
        stream = await navigator.mediaDevices.getUserMedia({ video: true });
      } catch (fallbackErr) {
        const msg =
          fallbackErr instanceof DOMException && fallbackErr.name === 'NotAllowedError'
            ? 'Camera permission was denied. Please allow camera access and try again.'
            : `Could not access camera: ${fallbackErr instanceof Error ? fallbackErr.message : String(fallbackErr)}`;
        setError(msg);
        setHasPermission(false);
        return;
      }
    }

    streamRef.current = stream;
    setHasPermission(true);

    if (videoRef.current) {
      videoRef.current.srcObject = stream;
      // Wait for metadata so width/height are available for captureSnapshot
      await videoRef.current.play().catch(() => {
        /* Some browsers auto-play — ignore the promise rejection */
      });
    }

    setIsStreaming(true);
  }, []);

  // --------------------------------------------------------------------------
  // stopCamera
  // --------------------------------------------------------------------------

  const stopCamera = useCallback(() => {
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }

    if (videoRef.current) {
      videoRef.current.srcObject = null;
    }

    // Revoke any previously created object URLs
    if (previewObjectUrlRef.current) {
      URL.revokeObjectURL(previewObjectUrlRef.current);
      previewObjectUrlRef.current = null;
    }

    setIsStreaming(false);
  }, []);

  // --------------------------------------------------------------------------
  // captureSnapshot
  // --------------------------------------------------------------------------

  const captureSnapshot = useCallback(async (): Promise<string | null> => {
    const video = videoRef.current;
    if (!video || !isStreaming) return null;

    setIsProcessing(true);

    try {
      const srcW = video.videoWidth;
      const srcH = video.videoHeight;

      if (srcW === 0 || srcH === 0) {
        setError('Video stream dimensions are not available yet.');
        return null;
      }

      // ── Step 1: crop to center square ────────────────────────────────────
      const size = Math.min(srcW, srcH);
      const cropX = Math.floor((srcW - size) / 2);
      const cropY = Math.floor((srcH - size) / 2);

      const canvas = document.createElement('canvas');
      canvas.width = size;
      canvas.height = size;

      const ctx = canvas.getContext('2d');
      if (!ctx) {
        setError('Canvas 2D context is not available.');
        return null;
      }

      ctx.drawImage(video, cropX, cropY, size, size, 0, 0, size, size);

      // ── Step 2: desaturate (luminance formula) ───────────────────────────
      const imageData = ctx.getImageData(0, 0, size, size);
      const pixels = imageData.data; // Uint8ClampedArray [R, G, B, A, …]

      for (let i = 0; i < pixels.length; i += 4) {
        const r = pixels[i];
        const g = pixels[i + 1];
        const b = pixels[i + 2];
        // BT.601 luminance coefficients
        const luma = Math.round(0.299 * r + 0.587 * g + 0.114 * b);
        pixels[i] = luma;
        pixels[i + 1] = luma;
        pixels[i + 2] = luma;
        // alpha (pixels[i + 3]) unchanged
      }

      // ── Step 3: contrast +20% (deviation from 128 × 1.2) ────────────────
      const CONTRAST_FACTOR = 1.2;
      const MID = 128;

      for (let i = 0; i < pixels.length; i += 4) {
        // Only need to adjust one channel because all three are identical after
        // desaturation, and we'll write the same value to all three.
        const adjusted = Math.round((pixels[i] - MID) * CONTRAST_FACTOR + MID);
        const clamped = Math.max(0, Math.min(255, adjusted));
        pixels[i] = clamped;
        pixels[i + 1] = clamped;
        pixels[i + 2] = clamped;
      }

      ctx.putImageData(imageData, 0, 0);

      // ── Encode as JPEG ────────────────────────────────────────────────────
      const dataUrl = canvas.toDataURL('image/jpeg', 0.92);

      setPreviewUrl(dataUrl);
      return dataUrl;
    } catch (err) {
      setError(
        `Snapshot failed: ${err instanceof Error ? err.message : String(err)}`,
      );
      return null;
    } finally {
      setIsProcessing(false);
    }
  }, [isStreaming]);

  // --------------------------------------------------------------------------

  return {
    videoRef,
    isStreaming,
    hasPermission,
    error,
    startCamera,
    stopCamera,
    captureSnapshot,
    previewUrl,
    isProcessing,
  };
}
