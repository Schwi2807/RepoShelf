import { closeDialog, useAppDialog } from '../../state/ui';
import { CloneDialog } from './CloneDialog';
import { ImportDialog } from './ImportDialog';

/** App-level dialogs opened from the "+" menu / empty states. Mounted once in App. */
export function DialogHost() {
  const d = useAppDialog();
  if (d === 'import') return <ImportDialog onClose={closeDialog} />;
  if (d === 'clone') return <CloneDialog onClose={closeDialog} />;
  return null;
}
