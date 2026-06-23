import { useState, useEffect, useCallback } from 'react';
import { api } from '../api';
import type { DateFormat } from '../dates';

export type CloseAction = 'ask' | 'minimize' | 'quit';
export type PinnedScope = 'perFolder' | 'global';
export type FolderColorStyle = 'icon' | 'bar' | 'row';
export type StartView = 'dashboard' | 'lastNote';

export interface DashboardWidget { key: string; w: 1 | 2; }

export interface AppSettings {
  startMinimized: boolean;
  dateFormat: DateFormat;
  pinnedScope: PinnedScope;
  folderColorStyle: FolderColorStyle;
  revisionLimit: number;
  autosaveDelay: number;
  startView: StartView;
  dashboardLayout: DashboardWidget[];
  compactTree: boolean;
  treeProgress: boolean;
  trashEnabled: boolean;
  trashRetentionDays: number;
  closeAction: CloseAction;
}

const DEFAULT_LAYOUT: DashboardWidget[] = [{ key: 'recent', w: 1 }, { key: 'due', w: 1 }, { key: 'stats', w: 1 }, { key: 'pinned', w: 1 }];

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
  trashEnabled: true,
  trashRetentionDays: 30,
  closeAction: 'ask',
};

export function parseLayout(raw: string | undefined): DashboardWidget[] {
  if (!raw) return DEFAULT_LAYOUT;
  try {
    const v = JSON.parse(raw);
    if (!Array.isArray(v)) return DEFAULT_LAYOUT;
    if (v.every((x: unknown) => typeof x === 'string')) return (v as string[]).map(k => ({ key: k, w: 1 as const }));
    if (v.every((x: unknown) => !!x && typeof (x as DashboardWidget).key === 'string' && ((x as DashboardWidget).w === 1 || (x as DashboardWidget).w === 2))) return v as DashboardWidget[];
    return DEFAULT_LAYOUT;
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
        trashEnabled: raw.trashEnabled !== 'false',
        trashRetentionDays: Number(raw.trashRetentionDays) > 0 ? Number(raw.trashRetentionDays) : 30,
        closeAction: (['minimize', 'quit'].includes(raw.closeAction) ? raw.closeAction : 'ask') as CloseAction,
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
