import { useEffect, useState } from "react";
import { api, type AppInfo } from "../api";

type Page = "about";

interface NavItemProps {
  label: string;
  active: boolean;
  onClick: () => void;
}

function NavItem({ label, active, onClick }: NavItemProps) {
  return (
    <button
      onClick={onClick}
      className={`w-full text-left px-4 py-2 text-sm transition-colors ${
        active
          ? "bg-gray-800 text-white"
          : "text-gray-400 hover:bg-gray-900 hover:text-gray-200"
      }`}
    >
      {label}
    </button>
  );
}

interface Props {
  onClose: () => void;
}

export default function Settings({ onClose }: Props) {
  const [page, setPage] = useState<Page>("about");
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    api.getAppInfo().then(setInfo);
  }, []);

  return (
    <div className="flex h-screen overflow-hidden">
      <aside className="w-52 shrink-0 bg-gray-950 flex flex-col h-full select-none">
        <div className="px-4 py-3 flex items-center justify-between border-b border-gray-800">
          <span className="text-gray-400 text-xs font-semibold uppercase tracking-widest">
            Settings
          </span>
          <button
            onClick={onClose}
            className="w-6 h-6 flex items-center justify-center text-gray-500 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title="Back to notes"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
        <nav className="flex-1 py-2">
          <NavItem label="About" active={page === "about"} onClick={() => setPage("about")} />
        </nav>
      </aside>

      <main className="flex-1 overflow-auto px-10 py-10" style={{ background: "#fef9c3" }}>
        {page === "about" && info && (
          <div>
            <h1 className="text-2xl font-bold text-gray-900 mb-1">{info.name}</h1>
            <p className="text-sm text-gray-500 mb-8">Version {info.version}</p>
            <p className="text-sm text-gray-600">{info.description}</p>
          </div>
        )}
      </main>
    </div>
  );
}
