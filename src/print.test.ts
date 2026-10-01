import { describe, it, expect, vi, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';

const loadOne = vi.hoisted(() => vi.fn());
vi.mock('./api', () => ({ api: { notes: { loadOne } } }));

import { sanitizeForPrint, printNoteHtml, printNote, clearPrintRoot } from './print';

const root = () => document.getElementById('print-root');

describe('sanitizeForPrint', () => {
  it('drops scripts, event handlers and javascript: urls', () => {
    const out = sanitizeForPrint('<p onclick="x()">hi</p><script>evil()</script><img src="javascript:x" onerror="y()"><a href="javascript:z">l</a><iframe src="https://e"></iframe>');
    expect(out).not.toMatch(/script|onclick|onerror|javascript:|iframe/i);
    expect(out).toContain('<p>hi</p>');
  });

  it('keeps ordinary content and disables checkboxes', () => {
    const out = sanitizeForPrint('<ul data-type="taskList"><li data-checked="true"><label><input type="checkbox" checked></label><div><p>t</p></div></li></ul><img src="noteimg://localhost/a.png"><a href="https://x.dev">x</a>');
    expect(out).toContain('src="noteimg://localhost/a.png"');
    expect(out).toContain('href="https://x.dev"');
    expect(out).toMatch(/<input[^>]*disabled/);
  });

  it('tolerates empty input', () => {
    expect(sanitizeForPrint('')).toBe('');
  });
});

describe('printNoteHtml', () => {
  beforeEach(() => {
    root()?.remove();
    window.print = vi.fn();
  });

  it('mounts the note into #print-root and prints', async () => {
    await printNoteHtml('<h1>Title</h1><p>Body</p>');
    expect(root()?.querySelector('.print-doc')?.innerHTML).toBe('<h1>Title</h1><p>Body</p>');
    expect(window.print).toHaveBeenCalledOnce();
  });

  it('replaces the previous print and clears on afterprint', async () => {
    await printNoteHtml('<p>one</p>');
    await printNoteHtml('<p>two</p>');
    expect(document.querySelectorAll('#print-root').length).toBe(1);
    expect(root()?.textContent).toBe('two');
    window.dispatchEvent(new Event('afterprint'));
    expect(root()?.childElementCount).toBe(0);
  });

  it('waits for pending images before printing', async () => {
    let print = false;
    window.print = vi.fn(() => { print = true; });
    const done = printNoteHtml('<img src="a.png">');
    await Promise.resolve();
    expect(print).toBe(false);
    root()!.querySelector('img')!.dispatchEvent(new Event('load'));
    await done;
    expect(print).toBe(true);
  });

  it('gives up waiting on images after a timeout', async () => {
    vi.useFakeTimers();
    try {
      const done = printNoteHtml('<img src="slow.png">');
      await vi.advanceTimersByTimeAsync(3000);
      await done;
      expect(window.print).toHaveBeenCalledOnce();
    } finally {
      vi.useRealTimers();
    }
  });

  it('swallows a rejected native print', async () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    window.print = vi.fn(() => Promise.reject(new Error('no printer'))) as unknown as typeof window.print;
    await expect(printNoteHtml('<p>x</p>')).resolves.toBeUndefined();
    expect(err).toHaveBeenCalled();
    err.mockRestore();
  });

  it('clearPrintRoot is a no-op without a root', () => {
    expect(() => clearPrintRoot()).not.toThrow();
  });
});

describe('printNote', () => {
  beforeEach(() => {
    root()?.remove();
    window.print = vi.fn();
    loadOne.mockReset();
  });

  it('prints the decrypted content from notes_load_one', async () => {
    loadOne.mockResolvedValue('<p>secret</p>');
    await printNote('n1');
    expect(loadOne).toHaveBeenCalledWith('n1');
    expect(root()?.textContent).toBe('secret');
    expect(window.print).toHaveBeenCalledOnce();
  });

  it('does not print when the note cannot be opened', async () => {
    const err = vi.spyOn(console, 'error').mockImplementation(() => {});
    loadOne.mockRejectedValue('vault locked');
    await printNote('n1');
    expect(window.print).not.toHaveBeenCalled();
    expect(err).toHaveBeenCalled();
    err.mockRestore();
  });
});

describe('capabilities', () => {
  // On macOS Tauri's `window.print` is an IPC call to `plugin:webview|print`,
  // which `core:default` does not grant: without this the button silently
  // does nothing.
  it('grants the webview print command', () => {
    const cap = JSON.parse(readFileSync('src-tauri/capabilities/default.json', 'utf8'));
    expect(cap.permissions).toContain('core:webview:allow-print');
  });
});
