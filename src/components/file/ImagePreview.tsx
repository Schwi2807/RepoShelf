// Image preview (FR-24, NFR-S7): bytes from `blob_bytes` → blob: URL (MIME from blob_meta) shown in an <img> on a
// checkerboard; SVG only ever through <img> (scripts in it never run). URLs are revoked on unmount.

import { ImageOff } from 'lucide-react';
import { useEffect, useState } from 'react';
import { blobBytes } from '../../ipc/api';
import { errorText } from '../../ipc/errors';
import { Shimmer } from '../ui/bits';

export function ImagePreview({ repoId, commit, path, mime, name }: { repoId: number; commit: string; path: string; mime: string; name: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [dims, setDims] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    let made: string | null = null;
    setUrl(null);
    setError(null);
    blobBytes(repoId, commit, path).then(
      (bytes) => {
        if (!alive) return;
        made = URL.createObjectURL(new Blob([bytes], { type: mime }));
        setUrl(made);
      },
      (e: unknown) => {
        if (alive) setError(errorText(e));
      },
    );
    return () => {
      alive = false;
      if (made) URL.revokeObjectURL(made);
    };
  }, [repoId, commit, path, mime]);

  if (error) {
    return (
      <div className="placeholder">
        <ImageOff size={32} aria-hidden="true" className="placeholder__icon" />
        <p>{error}</p>
      </div>
    );
  }
  return (
    <div className="imgview">
      <div className="imgview__stage checker">
        {url ? (
          <img
            src={url}
            alt={name}
            className="imgview__img"
            onLoad={(e) => setDims(`${e.currentTarget.naturalWidth} × ${e.currentTarget.naturalHeight} px`)}
          />
        ) : (
          <Shimmer width={160} height={120} />
        )}
      </div>
      {dims && <p className="imgview__dims small muted">{dims}</p>}
    </div>
  );
}
