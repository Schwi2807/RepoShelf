// File placeholders (FR-25): binary, too large, Git LFS pointer, submodule, symlink. Each says what it is and offers
// what is still possible (Save a copy, follow a symlink inside the repo).

import { FileArchive, FileSymlink, FileWarning, FileX, FolderGit2, HardDriveDownload, type LucideIcon } from 'lucide-react';
import type { ReactNode } from 'react';

export function Placeholder({
  icon: Icon,
  title,
  children,
  actions,
  kind,
}: {
  icon: LucideIcon;
  title: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
  kind: string;
}) {
  return (
    <div className="placeholder" data-placeholder={kind}>
      <Icon size={32} aria-hidden="true" className="placeholder__icon" />
      <p className="placeholder__title">{title}</p>
      {children && <div className="placeholder__body">{children}</div>}
      {actions && <div className="row placeholder__actions">{actions}</div>}
    </div>
  );
}

export const PLACEHOLDER_ICONS = {
  binary: FileX,
  large: FileWarning,
  lfs: HardDriveDownload,
  submodule: FolderGit2,
  symlink: FileSymlink,
  archive: FileArchive,
} as const;
