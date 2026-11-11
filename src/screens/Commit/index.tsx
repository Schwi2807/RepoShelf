// S6 Commit page — M5 placeholder. Full screen: M7.2.

import type { Route } from '../../router/routes';
import { RepoPlaceholder } from '../placeholder';

export default function Commit({ route }: { route: Route }) {
  return <RepoPlaceholder title="Commit" detail={route.params.oid} />;
}
