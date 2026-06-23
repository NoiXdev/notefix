import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { DateFormat } from '../dates';

export type PinnedScope = 'perFolder' | 'global';

export interface AppSettings {
  startMinimized: boolean;
  dateFormat: DateFormat;
  pinnedScope: PinnedScope;
}

const DEFAULTS: AppSettings = { startMinimized: false, dateFormat: 'auto', pinnedScope: 'perFolder' };

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULTS);

  useEffect(() => {
    api.settings.load().then(raw => {
      setSettings({
        startMinimized: raw.startMinimized === 'true',
        dateFormat: (['de', 'iso', 'us'].includes(raw.dateFormat) ? raw.dateFormat : 'auto') as DateFormat,
        pinnedScope: raw.pinnedScope === 'global' ? 'global' : 'perFolder',
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
