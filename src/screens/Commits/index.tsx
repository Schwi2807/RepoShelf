// S5 Commits tab — M5 placeholder. Full screen: M7.1.

import type { Route } from '../../router/routes';
import { RepoPlaceholder } from '../placeholder';

export default function Commits({ route }: { route: Route }) {
  return <RepoPlaceholder title="Commits" detail={route.query.path ? `history for ${route.query.path}` : undefined} />;
}
