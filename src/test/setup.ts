import '@testing-library/jest-dom';
import i18n from "../i18n";
// Keep the test UI language in German: force both i18n and the browser
// locale so App's resolveLang('system', navigator.language) stays 'de'
// (matches Playwright's locale: 'de-DE'). Without this, jsdom reports
// 'en-US' and App's language effect would switch the UI to English.
Object.defineProperty(navigator, 'language', { value: 'de-DE', configurable: true });
void i18n.changeLanguage("de");
