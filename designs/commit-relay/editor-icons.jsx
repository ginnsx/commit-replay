function EditorIcon({ kind, size = 18 }) {
  const s = size;
  const icons = {
    vscode: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <path fill="#0065A9" d="M2 2.5l12.5 7.5L2 17.5V2.5z"/>
        <path fill="#1F9CF0" d="M2 2.5l7 7.5-7 7.5V2.5z"/>
        <path fill="#fff" opacity=".9" d="M7.5 10l-2.2 2.2 1.4 1.4L10.3 10 6.7 6.4 5.3 7.8 7.5 10z"/>
      </svg>
    ),
    visualstudio: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <path fill="#52218A" d="M2 3.5l7 3.5v6l-7 3.5V3.5z"/>
        <path fill="#7B3FE4" d="M9 7l9-3.5v13L9 13V7z"/>
      </svg>
    ),
    cursor: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <rect width="20" height="20" rx="4" fill="#1a1a1a"/>
        <path fill="#e8e8e8" d="M5 5h4v10H5V5zm6 0h4v10h-4V5z"/>
      </svg>
    ),
    explorer: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <path fill="#F4B400" d="M3 5h5l1.5 2H17v9H3V5z"/>
        <path fill="#FDD663" d="M3 5h5l1 1.5H3V5z"/>
      </svg>
    ),
    terminal: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <rect width="20" height="20" rx="3" fill="#2d2d2d"/>
        <path stroke="#ccc" strokeWidth="1.5" d="M5 7l3 3-3 3M10 13h5"/>
      </svg>
    ),
    gitbash: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <rect width="20" height="20" rx="3" fill="#2d2d2d"/>
        <circle cx="10" cy="10" r="4" fill="#F05133"/>
        <path stroke="#fff" strokeWidth="1" d="M10 8v4M8.5 9.5h3"/>
      </svg>
    ),
    idea: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <rect width="20" height="20" rx="4" fill="#000"/>
        <path fill="#FE315D" d="M4 16l12-12 0 6-6 6z"/>
        <path fill="#fff" d="M6 14h8v2H6v-2z"/>
      </svg>
    ),
    pycharm: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <rect width="20" height="20" rx="4" fill="#21D789"/>
        <path fill="#087CFA" d="M4 16l8-12 4 4-8 12H4z"/>
        <path fill="#fff" d="M6 14h6v2H6v-2z"/>
      </svg>
    ),
    custom: (
      <svg width={s} height={s} viewBox="0 0 20 20" fill="none">
        <rect width="20" height="20" rx="4" fill="#444"/>
        <path stroke="#aaa" strokeWidth="1.5" d="M6 10h8M10 6v8"/>
      </svg>
    ),
  };
  return <span className="editor-icon">{icons[kind] || icons.custom}</span>;
}

Object.assign(window, { EditorIcon });
