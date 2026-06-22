import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';

export type PinnedDisplayMode = 'flat' | 'sections';

export interface AppSettings {
  pinnedDisplayMode: PinnedDisplayMode;
  startMinimized: boolean;
}

const DEFAULTS: AppSettings = { pinnedDisplayMode: 'flat', startMinimized: false };

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULTS);

  useEffect(() => {
    api.settings.load().then(raw => {
      setSettings({
        pinnedDisplayMode: raw.pinnedDisplayMode === 'sections' ? 'sections' : 'flat',
        startMinimized: raw.startMinimized === 'true',
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
