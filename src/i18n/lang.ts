export type LangSetting = 'system' | 'en' | 'de' | 'fr';
export type Lang = 'en' | 'de' | 'fr';

export function resolveLang(setting: LangSetting, sysLang: string): Lang {
  if (setting === 'de' || setting === 'en' || setting === 'fr') return setting;
  const l = sysLang.toLowerCase();
  if (l.startsWith('de')) return 'de';
  if (l.startsWith('fr')) return 'fr';
  return 'en';
}
