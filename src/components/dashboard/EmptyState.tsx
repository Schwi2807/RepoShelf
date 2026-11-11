// Empty shelf (S1): illustration-free card with Import / Clone.

import { Download, FolderPlus, Library } from 'lucide-react';
import { Box } from '../layout/Sidebar';
import { Button } from '../ui/Button';
import { Empty } from '../ui/bits';
import { openDialog } from '../../state/ui';

export function EmptyState() {
  return (
    <Box className="empty-card">
      <Empty
        icon={Library}
        title="Your shelf is empty"
        actions={
          <>
            <Button icon={FolderPlus} onClick={() => openDialog('import')}>
              Import folder…
            </Button>
            <Button variant="primary" icon={Download} onClick={() => openDialog('clone')}>
              Clone URL…
            </Button>
          </>
        }
      >
        Import a git repository from this PC (it stays where it is, read-only) or clone a public https:// repository into
        your archive.
      </Empty>
    </Box>
  );
}
