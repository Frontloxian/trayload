const PATHS: Record<string, string> = {
  home: "M3 11.5 12 4l9 7.5M5.5 10v9.5h4.5v-5.5h4v5.5h4.5V10",
  library: "M4 4h6.5v6.5H4zM13.5 4H20v6.5h-6.5zM4 13.5h6.5V20H4zM13.5 13.5H20V20h-6.5z",
  emulators: "M12 3.5v11m0 0-4-4m4 4 4-4M4.5 15.5v3a1.5 1.5 0 0 0 1.5 1.5h12a1.5 1.5 0 0 0 1.5-1.5v-3",
  setup: "M9.5 6.5H20M9.5 12H20M9.5 17.5H20M4 6.5l1.2 1.2L7.4 5.3M4 12l1.2 1.2L7.4 10.8M4 17.5l1.2 1.2 2.2-2.4",
  news: "M5 4.5h11a1 1 0 0 1 1 1V19a1.5 1.5 0 0 0 3 0V9h-3M5 4.5V18a1.5 1.5 0 0 0 1.5 1.5H18M8 8.5h6M8 12h6M8 15.5h3.5",
  settings: "M4 7h8m4 0h4M4 17h4m4 0h8M12 7a2 2 0 1 0 4 0 2 2 0 0 0-4 0ZM8 17a2 2 0 1 0 4 0 2 2 0 0 0-4 0Z",
  about: "M12 3.5a8.5 8.5 0 1 0 0 17 8.5 8.5 0 0 0 0-17ZM12 11v5.5M12 7.8v.1",
  quick: "M13 3 5 13.5h6L10 21l8-10.5h-6z",
};

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.7"
      strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  );
}
