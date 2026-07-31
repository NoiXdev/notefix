import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faArrowUpRightFromSquare, faXmark } from '@fortawesome/free-solid-svg-icons';
import type { UpdateInfo } from '../api';

export default function UpdateBanner({ info, onDownload, onDismiss }: {
  info: UpdateInfo;
  onDownload: () => void;
  onDismiss: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="flex items-center gap-3 px-4 py-2 text-sm bg-[var(--panel)] border-b border-[var(--line)] text-[var(--ink)]">
      <span className="flex-1">{t('update.bannerText', { version: info.latest })}</span>
      <button onClick={onDownload} className="inline-flex items-center gap-1.5 font-medium underline hover:no-underline">
        <FontAwesomeIcon icon={faArrowUpRightFromSquare} className="text-xs" />
        {t('update.download')}
      </button>
      <button onClick={onDismiss} aria-label={t('update.dismiss')} title={t('update.dismiss')}
        className="p-1 rounded hover:bg-[var(--line)]/70">
        <FontAwesomeIcon icon={faXmark} />
      </button>
    </div>
  );
}
