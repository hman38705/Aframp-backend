'use client';

/**
 * FileUploadComponent — task #481 step 5
 *
 * Drag-and-drop + click-to-browse file upload zone for KYC document uploads.
 *
 * Features:
 * - MIME-type allow-list check (client-side)
 * - File-size limit check (client-side)
 * - Image compression: canvas resize to ≤1200 px on longest side, JPEG 0.8,
 *   triggered when the file is image/* AND > 1 MB
 * - Non-image files (PDF etc.) read as-is via FileReader.readAsDataURL
 * - Drag-active visual state
 * - Inline error display
 * - Accessible (role="button", aria-label, hidden file input)
 * - No external UI library — pure Tailwind CSS
 */

import { useRef, useState, useCallback, type DragEvent, type ChangeEvent } from 'react';

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

export interface FileUploadComponentProps {
  /** Unique field identifier — forwarded to the hidden <input id>. */
  fieldId: string;
  /**
   * Allowed MIME types, e.g. ["image/jpeg", "image/png", "application/pdf"].
   * At least one must match `file.type` for the file to be accepted.
   */
  accept: string[];
  /** Files larger than this value (in megabytes) are rejected. */
  maxSizeMb: number;
  /** Called with the (possibly compressed) File and its data URL on success. */
  onFileSelected: (file: File, dataUrl: string) => void;
  /** Called with a human-readable error string on any rejection. */
  onError: (error: string) => void;
  /** Disables the upload zone when true. */
  disabled?: boolean;
  /** Name of the currently selected file — shown in the zone when set. */
  currentFileName?: string;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const ONE_MB = 1024 * 1024;
const MAX_IMAGE_DIMENSION = 1200;
const JPEG_QUALITY = 0.8;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Resize an image file to ≤ MAX_IMAGE_DIMENSION px on its longest side. */
async function compressImage(file: File): Promise<File> {
  return new Promise<File>((resolve, reject) => {
    const img = new Image();
    const objectUrl = URL.createObjectURL(file);

    img.onload = () => {
      URL.revokeObjectURL(objectUrl);

      let { width, height } = img;

      if (width <= MAX_IMAGE_DIMENSION && height <= MAX_IMAGE_DIMENSION) {
        // Already within limits — no resize needed
        resolve(file);
        return;
      }

      const scale =
        width >= height
          ? MAX_IMAGE_DIMENSION / width
          : MAX_IMAGE_DIMENSION / height;

      width = Math.round(width * scale);
      height = Math.round(height * scale);

      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;

      const ctx = canvas.getContext('2d');
      if (!ctx) {
        reject(new Error('Canvas 2D context unavailable'));
        return;
      }

      ctx.drawImage(img, 0, 0, width, height);

      canvas.toBlob(
        (blob) => {
          if (!blob) {
            reject(new Error('Image compression produced an empty blob'));
            return;
          }
          const compressed = new File([blob], file.name, {
            type: 'image/jpeg',
            lastModified: Date.now(),
          });
          resolve(compressed);
        },
        'image/jpeg',
        JPEG_QUALITY,
      );
    };

    img.onerror = () => {
      URL.revokeObjectURL(objectUrl);
      reject(new Error('Image failed to load for compression'));
    };

    img.src = objectUrl;
  });
}

/** Read a File as a base-64 data URL. */
function readAsDataUrl(file: File): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(new Error('FileReader failed'));
    reader.readAsDataURL(file);
  });
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function FileUploadComponent({
  fieldId,
  accept,
  maxSizeMb,
  onFileSelected,
  onError,
  disabled = false,
  currentFileName,
}: FileUploadComponentProps) {
  const inputRef = useRef<HTMLInputElement>(null);

  const [isDragActive, setIsDragActive] = useState(false);
  const [inlineError, setInlineError] = useState<string | null>(null);
  const [selectedName, setSelectedName] = useState<string | null>(
    currentFileName ?? null,
  );
  const [isProcessing, setIsProcessing] = useState(false);

  // --------------------------------------------------------------------------
  // File processing pipeline
  // --------------------------------------------------------------------------

  const processFile = useCallback(
    async (file: File) => {
      setInlineError(null);

      // MIME-type check
      if (!accept.includes(file.type)) {
        const msg = `File type "${file.type}" is not allowed. Accepted: ${accept.join(', ')}.`;
        setInlineError(msg);
        onError(msg);
        return;
      }

      // Size check
      if (file.size > maxSizeMb * ONE_MB) {
        const msg = `File is too large (${(file.size / ONE_MB).toFixed(1)} MB). Maximum allowed: ${maxSizeMb} MB.`;
        setInlineError(msg);
        onError(msg);
        return;
      }

      setIsProcessing(true);

      try {
        let fileToRead = file;

        // Image compression (only when > 1 MB)
        if (file.type.startsWith('image/') && file.size > ONE_MB) {
          fileToRead = await compressImage(file);
        }

        const dataUrl = await readAsDataUrl(fileToRead);
        setSelectedName(fileToRead.name);
        onFileSelected(fileToRead, dataUrl);
      } catch (err) {
        const msg = `Failed to process file: ${err instanceof Error ? err.message : String(err)}`;
        setInlineError(msg);
        onError(msg);
      } finally {
        setIsProcessing(false);
      }
    },
    [accept, maxSizeMb, onError, onFileSelected],
  );

  // --------------------------------------------------------------------------
  // Drag handlers
  // --------------------------------------------------------------------------

  const handleDragOver = useCallback(
    (e: DragEvent<HTMLDivElement>) => {
      if (disabled) return;
      e.preventDefault();
      e.stopPropagation();
      setIsDragActive(true);
    },
    [disabled],
  );

  const handleDragLeave = useCallback((e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragActive(false);
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent<HTMLDivElement>) => {
      if (disabled) return;
      e.preventDefault();
      e.stopPropagation();
      setIsDragActive(false);

      const file = e.dataTransfer.files?.[0];
      if (file) void processFile(file);
    },
    [disabled, processFile],
  );

  // --------------------------------------------------------------------------
  // Click-to-browse
  // --------------------------------------------------------------------------

  const handleButtonClick = useCallback(() => {
    if (!disabled) inputRef.current?.click();
  }, [disabled]);

  const handleInputChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (file) void processFile(file);
      // Reset so re-selecting the same file fires onChange again
      e.target.value = '';
    },
    [processFile],
  );

  // --------------------------------------------------------------------------
  // Derived styles
  // --------------------------------------------------------------------------

  const zoneBase =
    'flex flex-col items-center justify-center gap-2 rounded-lg border-2 border-dashed p-6 transition-colors duration-150 cursor-pointer text-center select-none';

  const zoneStyle = disabled
    ? `${zoneBase} border-gray-200 bg-gray-50 cursor-not-allowed opacity-60`
    : isDragActive
      ? `${zoneBase} border-blue-500 bg-blue-50`
      : inlineError
        ? `${zoneBase} border-red-400 bg-red-50`
        : selectedName
          ? `${zoneBase} border-green-400 bg-green-50`
          : `${zoneBase} border-gray-300 bg-white hover:border-blue-400 hover:bg-blue-50`;

  // --------------------------------------------------------------------------

  return (
    <div className="w-full">
      {/* Hidden file input */}
      <input
        ref={inputRef}
        id={fieldId}
        type="file"
        accept={accept.join(',')}
        autoComplete="off"
        tabIndex={-1}
        className="sr-only"
        disabled={disabled}
        onChange={handleInputChange}
        aria-hidden="true"
      />

      {/* Drop zone */}
      <div
        role="button"
        tabIndex={disabled ? -1 : 0}
        aria-label={
          selectedName
            ? `Replace file: ${selectedName}. Accepted types: ${accept.join(', ')}. Maximum size: ${maxSizeMb} MB.`
            : `Upload file. Accepted types: ${accept.join(', ')}. Maximum size: ${maxSizeMb} MB. Click or drag a file here.`
        }
        aria-disabled={disabled}
        className={zoneStyle}
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
        onClick={handleButtonClick}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            handleButtonClick();
          }
        }}
      >
        {/* Upload icon */}
        <svg
          xmlns="http://www.w3.org/2000/svg"
          className={`w-8 h-8 ${inlineError ? 'text-red-400' : selectedName ? 'text-green-500' : 'text-gray-400'}`}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth={1.5}
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M4 17v2a2 2 0 002 2h12a2 2 0 002-2v-2" />
          <polyline points="16 12 12 8 8 12" />
          <line x1="12" y1="8" x2="12" y2="20" />
        </svg>

        {/* Primary text */}
        <p className="text-sm font-medium text-gray-700">
          {isProcessing
            ? 'Processing…'
            : isDragActive
              ? 'Drop your file here'
              : selectedName
                ? selectedName
                : 'Drag & drop or click to browse'}
        </p>

        {/* Hint */}
        {!isProcessing && (
          <p className="text-xs text-gray-500">
            {accept.join(', ')} · max {maxSizeMb} MB
          </p>
        )}
      </div>

      {/* Inline error */}
      {inlineError && (
        <p role="alert" className="mt-1.5 text-xs text-red-600">
          {inlineError}
        </p>
      )}
    </div>
  );
}
