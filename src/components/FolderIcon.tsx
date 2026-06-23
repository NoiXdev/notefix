import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { FA_BY_NAME } from '../folderIcons';

interface Props {
  icon: string;
  tint?: string;
  size?: number;
}

export default function FolderIcon({ icon, tint, size = 13 }: Props) {
  if (icon.startsWith('fa:')) {
    const def = FA_BY_NAME[icon.slice(3)];
    if (def) return <FontAwesomeIcon icon={def} style={{ color: tint, width: size, height: size }} />;
  } else if (icon) {
    return <span style={{ fontSize: size, lineHeight: 1 }}>{icon}</span>;
  }
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke={tint ?? 'currentColor'} strokeWidth="2" aria-hidden>
      <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    </svg>
  );
}
