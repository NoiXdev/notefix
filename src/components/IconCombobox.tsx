import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { searchIcons } from '../folderIcons';
import FolderIcon from './FolderIcon';

interface Props {
  value: string;
  onPick: (icon: string) => void;
}

export default function IconCombobox({ value, onPick }: Props) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const results = searchIcons(query);
  return (
    <div>
      <input
        autoFocus
        value={query}
        onChange={e => setQuery(e.target.value)}
        placeholder={t('folder.iconSearch')}
        className="w-full bg-gray-800 text-gray-100 text-sm px-2 py-1 rounded outline-none mb-2"
      />
      <div className="max-h-44 overflow-y-auto rounded border border-gray-700 divide-y divide-gray-800">
        {results.map(name => {
          const id = `fa:${name}`;
          const active = value === id;
          return (
            <button
              key={name}
              onClick={() => onPick(id)}
              className={`w-full flex items-center gap-2 px-2 py-1.5 text-sm text-left hover:bg-gray-700 ${active ? 'bg-gray-700' : ''}`}
            >
              <FolderIcon icon={id} size={14} />
              <span className="truncate">{name}</span>
            </button>
          );
        })}
        {results.length === 0 && <div className="px-2 py-2 text-xs text-gray-500">{t('folder.noResults')}</div>}
      </div>
    </div>
  );
}
