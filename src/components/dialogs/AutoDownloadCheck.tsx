// "Download the latest release for this PC" checkbox of the Clone and Import dialogs (v0.2.0 F4, D-112). Default =
// the setting `autoDownloadLatestRelease`; untouched it sends `null` (the backend applies the setting), touched it
// sends the choice. Only shown for github.com origins (the only ones with release files).

import { useState } from 'react';
import { useSettings } from '../../state/settings';

export function isGithubUrl(url: string | null | undefined): boolean {
  if (!url) return false;
  try {
    const u = new URL(url);
    return u.protocol === 'https:' && u.hostname.toLowerCase() === 'github.com';
  } catch {
    return false;
  }
}

/** The checkbox's state: `value` for the IPC call (null = untouched → the setting), `checked` for display. */
export function useAutoDownload(): { value: boolean | null; checked: boolean; set: (v: boolean) => void } {
  const { settings } = useSettings();
  const [choice, setChoice] = useState<boolean | null>(null);
  return { value: choice, checked: choice ?? settings.autoDownloadLatestRelease, set: setChoice };
}

export function AutoDownloadCheck({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="check-row autodl" data-testid="autodl">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span>
        <strong>Download the latest release for this PC</strong>
        <span className="muted small">
          After it is added, RepoShelf asks GitHub for the release files and downloads the one made for Windows on this PC (never
          run or unpacked). Default from Settings → Network.
        </span>
      </span>
    </label>
  );
}
