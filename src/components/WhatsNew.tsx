import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faXmark, faArrowUpRightFromSquare } from '@fortawesome/free-solid-svg-icons';
import { api, type ReleaseInfo } from '../api';
import { renderMarkdownSafe } from '../safeMarkdown';

interface Props {
  releases: ReleaseInfo[];
  onClose: () => void;
}

const REPO_RELEASES_URL = 'https://github.com/NoiXdev/notefix/releases/tag/';

function formatPublishedAt(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' });
}

/**
 * Cumulative changelog dialog: shown after an update (see App.tsx) or from
 * the About page's "What's New" link. Works on desktop and mobile alike —
 * it's just a network fetch (api.githubReleases) + this dialog, independent
 * of the desktop-only updater.
 */
export default function WhatsNew({ releases, onClose }: Props) {
  const { t } = useTranslation();
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ background: 'rgba(0,0,0,0.45)', paddingTop: 'env(safe-area-inset-top)', paddingBottom: 'env(safe-area-inset-bottom)' }}
      onClick={onClose}
    >
      <div
        className="w-[32rem] max-w-[94vw] max-h-[85vh] rounded-lg bg-gray-900 border border-gray-700 flex flex-col overflow-hidden"
        onClick={e => e.stopPropagation()}
      >
        <div className="flex items-center justify-between gap-3 px-5 pt-5 pb-3 shrink-0">
          <h2 className="text-gray-100 text-base font-semibold">{t('whatsNew.title')}</h2>
          <button
            onClick={onClose}
            aria-label={t('whatsNew.close')}
            title={t('whatsNew.close')}
            className="p-1 rounded text-gray-400 hover:text-gray-100 hover:bg-gray-800"
          >
            <FontAwesomeIcon icon={faXmark} />
          </button>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto px-5">
          {releases.length === 0 ? (
            <p className="text-sm text-gray-400 pb-5">{t('whatsNew.empty')}</p>
          ) : (
            <div className="flex flex-col divide-y divide-gray-800">
              {releases.map(r => (
                <div key={r.tagName} className="py-4 first:pt-0 last:pb-5">
                  <div className="flex items-baseline justify-between gap-3 flex-wrap mb-1.5">
                    <h3 className="text-gray-100 text-sm font-semibold">{r.name || r.tagName}</h3>
                    <span className="text-xs text-gray-500 whitespace-nowrap">{formatPublishedAt(r.publishedAt)}</span>
                  </div>
                  <div className="whatsnew-body text-sm text-gray-300" dangerouslySetInnerHTML={{ __html: renderMarkdownSafe(r.body || '') }} />
                  <button
                    onClick={() => void api.openExternal(`${REPO_RELEASES_URL}${encodeURIComponent(r.tagName)}`)}
                    className="mt-1.5 inline-flex items-center gap-1.5 text-xs text-blue-400 hover:text-blue-300 underline"
                  >
                    <FontAwesomeIcon icon={faArrowUpRightFromSquare} className="text-[10px]" />
                    {t('whatsNew.viewOnGitHub')}
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex justify-end gap-2 px-5 py-4 border-t border-gray-800 shrink-0">
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded text-sm font-medium"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('whatsNew.close')}
          </button>
        </div>
      </div>
    </div>
  );
}
