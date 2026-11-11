// S4 File view — M5 placeholder. Full screen: M6.7.

import type { Route } from '../../router/routes';
import { RepoPlaceholder } from '../placeholder';

export default function FileView({ route }: { route: Route }) {
  const lines = route.query.L ? ` · lines ${route.query.L}` : '';
  return <RepoPlaceholder title="File" detail={`${route.params.path ?? ''}${lines}`} />;
}
