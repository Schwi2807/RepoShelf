// S10 Settings page — M5 placeholder (folders from `app_info`). Full screen: M7.6.

import { Box } from '../../components/layout/Sidebar';
import { Shimmer } from '../../components/ui/bits';
import { appInfo } from '../../ipc/api';
import { qk, useQuery } from '../../ipc/query';

export default function Settings() {
  const info = useQuery(qk('app_info'), () => appInfo());
  return (
    <main className="container page">
      <h1 className="page-title">Settings</h1>
      <Box header={<span className="box__title">Folders</span>}>
        <div className="box__body">
          {info.data ? (
            <dl className="kv">
              <dt className="label">Data folder</dt>
              <dd className="mono muted">{info.data.dataDir}</dd>
              <dt className="label">Archive folder</dt>
              <dd className="mono muted">{info.data.archiveDir}</dd>
            </dl>
          ) : (
            <Shimmer width={320} />
          )}
        </div>
      </Box>
    </main>
  );
}
