/** RepoShelf app mark: three book spines on a shelf (same design as docs/icon/reposhelf.svg; R-11, no GitHub marks). */
export function AppMark({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 1024 1024" aria-hidden="true" focusable="false">
      <rect x="32" y="32" width="960" height="960" rx="208" fill="#1d3b5c" />
      <rect x="236" y="260" width="150" height="476" rx="22" fill="#2bb3a3" />
      <rect x="268" y="320" width="86" height="22" rx="11" fill="#e8f6f3" />
      <rect x="410" y="200" width="160" height="536" rx="22" fill="#6c6fe0" />
      <rect x="444" y="262" width="92" height="22" rx="11" fill="#eceeff" />
      <rect x="444" y="306" width="92" height="22" rx="11" fill="#eceeff" />
      <g transform="rotate(12 690 736)">
        <rect x="614" y="296" width="140" height="440" rx="22" fill="#f0a93b" />
        <rect x="644" y="356" width="80" height="22" rx="11" fill="#fff4e0" />
      </g>
      <rect x="176" y="736" width="672" height="64" rx="20" fill="#d9e4ef" />
    </svg>
  );
}
