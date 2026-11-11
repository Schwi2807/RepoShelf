import { Compass } from 'lucide-react';
import { Box } from '../../components/layout/Sidebar';
import { Empty } from '../../components/ui/bits';
import { href } from '../../router/router';
import { Link } from '../../router/Link';

export default function NotFound() {
  return (
    <main className="container page">
      <Box>
        <Empty icon={Compass} title="Nothing here" actions={<Link to={href('dashboard')}>Back to all repositories</Link>}>
          This address does not match any page in RepoShelf.
        </Empty>
      </Box>
    </main>
  );
}
