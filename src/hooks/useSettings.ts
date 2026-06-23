import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { DateFormat } from '../dates';

export type PinnedScope = 'perFolder' | 'global';
export type FolderColorStyle = 'icon' | 'bar' | 'row';
export type StartView = 'dashboard' | 'lastNote';

export interface AppSettings {
  startMinimized: boolean;
  dateFormat: DateFormat;
  pinnedScope: PinnedScope;
  folderColorStyle: FolderColorStyle;
  revisionLimit: number;
  autosaveDelay: number;
  startView: StartView;
  dashboardLayout: string[];
  compactTree: boolean;
  treeProgress: boolean;
}

const DEFAULT_LAYOUT = ['recent', 'due', 'stats', 'pinned'];

const DEFAULTS: AppSettings = {
  startMinimized: false,
  dateFormat: 'auto',
  pinnedScope: 'perFolder',
  folderColorStyle: 'icon',
  revisionLimit: 50,
  autosaveDelay: 400,
  startView: 'lastNote',
  dashboardLayout: DEFAULT_LAYOUT,
  compactTree: false,
  treeProgress: true,
};

function parseLayout(raw: string | undefined): string[] {
  if (!raw) return DEFAULT_LAYOUT;
  try {
    const v = JSON.parse(raw);
    return Array.isArray(v) && v.every(x => typeof x === 'string') ? v : DEFAULT_LAYOUT;
  } catch {
    return DEFAULT_LAYOUT;
  }
}

export function useSettings() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULTS);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    api.settings.load().then(raw => {
      setSettings({
        startMinimized: raw.startMinimized === 'true',
        dateFormat: (['de', 'iso', 'us'].includes(raw.dateFormat) ? raw.dateFormat : 'auto') as DateFormat,
        pinnedScope: raw.pinnedScope === 'global' ? 'global' : 'perFolder',
        folderColorStyle: (['bar', 'row'].includes(raw.folderColorStyle) ? raw.folderColorStyle : 'icon') as FolderColorStyle,
        revisionLimit: Number(raw.revisionLimit) > 0 ? Number(raw.revisionLimit) : 50,
        autosaveDelay: Number(raw.autosaveDelay) >= 100 ? Number(raw.autosaveDelay) : 400,
        startView: raw.startView === 'dashboard' ? 'dashboard' : 'lastNote',
        dashboardLayout: parseLayout(raw.dashboardLayout),
        compactTree: raw.compactTree === 'true',
        treeProgress: raw.treeProgress !== 'false',
      });
      setLoaded(true);
    });
  }, []);

  const setSetting = useCallback(
    async <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
      setSettings(prev => ({ ...prev, [key]: value }));
      const serialized = Array.isArray(value) ? JSON.stringify(value) : String(value);
      await api.settings.set(key, serialized);
    },
    [],
  );

  return { settings, setSetting, loaded };
}
