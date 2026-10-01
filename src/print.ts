/**
 * Print a single note through the WebView's native print dialog.
 *
 * The note is rendered into a `#print-root` container under <body>, and print
 * CSS (index.css) hides everything else. It has to be the main document: on
 * macOS Tauri replaces `window.print` with a native print operation of the
 * whole webview, so an iframe's own `print()` would do nothing there.
 *
 * That operation is non-blocking — `window.print()` returns while the dialog
 * is still open — so the content stays until `afterprint` (or the next print)
 * replaces it, instead of being torn down right after the call.
 *
 * Desktop only: Android/iOS WebViews have no `window.print`; the UI gates on
 * `isMobilePlatform`.
 */

import { api } from './api';

const ROOT_ID = 'print-root';
const IMAGE_TIMEOUT_MS = 3000;

/** Strip anything executable from note HTML before it is mounted. Note
 *  content can arrive through sync, so it is not trusted to be inert. */
export function sanitizeForPrint(html: string): string {
  const doc = new DOMParser().parseFromString(html || '', 'text/html');
  doc.querySelectorAll('script, iframe, object, embed, link, meta, base, form').forEach(el => el.remove());
  doc.body.querySelectorAll('*').forEach(el => {
    for (const attr of Array.from(el.attributes)) {
      const name = attr.name.toLowerCase();
      if (name.startsWith('on')) el.removeAttribute(attr.name);
      else if ((name === 'href' || name === 'src') && /^\s*javascript:/i.test(attr.value)) el.removeAttribute(attr.name);
    }
  });
  // Printed checkboxes must not be toggled while the dialog is open.
  doc.body.querySelectorAll('input').forEach(el => el.setAttribute('disabled', ''));
  return doc.body.innerHTML;
}

function printRoot(): HTMLElement {
  let root = document.getElementById(ROOT_ID);
  if (!root) {
    root = document.createElement('div');
    root.id = ROOT_ID;
    root.setAttribute('aria-hidden', 'true');
    document.body.appendChild(root);
  }
  return root;
}

export function clearPrintRoot(): void {
  document.getElementById(ROOT_ID)?.replaceChildren();
}

/** Resolve once every image has loaded or failed (or after a timeout), so a
 *  page doesn't print with empty boxes where pictures should be. */
function imagesSettled(root: HTMLElement): Promise<void> {
  const pending = Array.from(root.querySelectorAll('img')).filter(img => !img.complete);
  if (pending.length === 0) return Promise.resolve();
  const all = Promise.all(pending.map(img => new Promise<void>(resolve => {
    img.addEventListener('load', () => resolve(), { once: true });
    img.addEventListener('error', () => resolve(), { once: true });
  })));
  return Promise.race([all.then(() => undefined), new Promise<void>(r => setTimeout(r, IMAGE_TIMEOUT_MS))]);
}

export async function printNoteHtml(html: string): Promise<void> {
  const root = printRoot();
  const doc = document.createElement('div');
  doc.className = 'ProseMirror print-doc';
  doc.innerHTML = sanitizeForPrint(html);
  root.replaceChildren(doc);
  window.addEventListener('afterprint', clearPrintRoot, { once: true });
  await imagesSettled(root);
  try {
    // On macOS this is Tauri's IPC shim and returns a promise.
    await window.print();
  } catch (e) {
    console.error('print failed', e);
  }
}

/** Print a stored note. Goes through `notes_load_one` so a protected note is
 *  decrypted (and refused while the vault is locked) like any other read. */
export async function printNote(id: string): Promise<void> {
  try {
    await printNoteHtml(await api.notes.loadOne(id));
  } catch (e) {
    console.error('print failed', e);
  }
}
