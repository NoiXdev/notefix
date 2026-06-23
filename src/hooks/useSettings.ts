import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { DateFormat } from '../dates';

export type PinnedScope = 'perFolder' | 'global';
export type FolderColorStyle = 'icon' | 'bar' | 'row';

export interface AppSettings {
  startMinimized: boolean;
  dateFormat: DateFormat;
  pinnedScope: PinnedScope;
  folderColorStyle: FolderColorStyle;
}

const DEFAULTS: AppSettings = { startMinimized: false, dateFormat: 'auto', pinnedScope: 'perFolder', folderColorStyle: 'icon' };

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULTS);

  useEffect(() => {
    api.settings.load().then(raw => {
      setSettings({
        startMinimized: raw.startMinimized === 'true',
        dateFormat: (['de', 'iso', 'us'].includes(raw.dateFormat) ? raw.dateFormat : 'auto') as DateFormat,
        pinnedScope: raw.pinnedScope === 'global' ? 'global' : 'perFolder',
        folderColorStyle: (['bar', 'row'].includes(raw.folderColorStyle) ? raw.folderColorStyle : 'icon') as FolderColorStyle,
      });
    });
  }, []);

  const setSetting = useCallback(
    async <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      setSettings(prev => ({ ...prev, [key]: value }));
      await api.settings.set(key, String(value));
    },
    [],
  );

  return { settings, setSetting };
}
