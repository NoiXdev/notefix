import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { ShowInvisibles, invisibleDecorationsFor, invisiblesKey } from './showInvisibles';

describe('invisibleDecorationsFor (pure)', () => {
  it('returns no ranges for an empty string', () => {
    expect(invisibleDecorationsFor('')).toEqual([]);
  });

  it('returns no ranges when there are no spaces', () => {
    expect(invisibleDecorationsFor('hello')).toEqual([]);
  });

  it('finds a single space', () => {
    expect(invisibleDecorationsFor('a b')).toEqual([[1, 2]]);
  });

  it('finds every space in a run of consecutive spaces', () => {
    expect(invisibleDecorationsFor('a   b')).toEqual([[1, 2], [2, 3], [3, 4]]);
  });

  it('finds multiple separated spaces', () => {
    expect(invisibleDecorationsFor('one two three')).toEqual([[3, 4], [7, 8]]);
  });

  it('finds a leading space', () => {
    expect(invisibleDecorationsFor(' leading')).toEqual([[0, 1]]);
  });

  it('finds trailing spaces', () => {
    expect(invisibleDecorationsFor('trailing  ')).toEqual([[8, 9], [9, 10]]);
  });

  it('does not treat a tab as a space', () => {
    expect(invisibleDecorationsFor('a\tb')).toEqual([]);
  });

  it('does not treat a non-breaking space (U+00A0) as a plain space', () => {
    expect(invisibleDecorationsFor('a b')).toEqual([]);
  });

  it('handles unicode text around ascii spaces correctly', () => {
    expect(invisibleDecorationsFor('café au lait')).toEqual([[4, 5], [7, 8]]);
  });

  it('a string of only spaces marks every position', () => {
    expect(invisibleDecorationsFor('   ')).toEqual([[0, 1], [1, 2], [2, 3]]);
  });
});

// --- Plugin wiring: exercised against a real headless TipTap Editor. ---
describe('ShowInvisibles extension (real Editor)', () => {
  const editors: Editor[] = [];
  function makeEditor(content: string) {
    const ed = new Editor({ extensions: [StarterKit, ShowInvisibles], content });
    editors.push(ed);
    return ed;
  }
  afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; });

  it('starts disabled with no decorations', () => {
    const ed = makeEditor('<p>a b</p>');
    expect(invisiblesKey.getState(ed.state)?.enabled).toBe(false);
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(0);
  });

  it('setInvisibles(true) decorates every space', () => {
    const ed = makeEditor('<p>one two three</p>');
    ed.commands.setInvisibles(true);
    expect(invisiblesKey.getState(ed.state)?.enabled).toBe(true);
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(2);
  });

  it('setInvisibles(false) removes decorations again', () => {
    const ed = makeEditor('<p>a b c</p>');
    ed.commands.setInvisibles(true);
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(2);
    ed.commands.setInvisibles(false);
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(0);
    expect(invisiblesKey.getState(ed.state)?.enabled).toBe(false);
  });

  it('renders a hard-break widget with the ↵ glyph', () => {
    const ed = makeEditor('<p>a<br>b</p>');
    ed.commands.setInvisibles(true);
    const widgets = ed.view.dom.querySelectorAll('.inv-break');
    expect(widgets.length).toBe(1);
    expect(widgets[0].textContent).toBe('↵');
  });

  it('does not decorate hard breaks or spaces while disabled', () => {
    const ed = makeEditor('<p>a b<br>c</p>');
    expect(ed.view.dom.querySelectorAll('.inv-space, .inv-break').length).toBe(0);
  });

  it('recomputes decorations when the doc changes while enabled', () => {
    const ed = makeEditor('<p>no spaces here</p>');
    ed.commands.setInvisibles(true);
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(2);
    ed.commands.setContent('<p>a b c d</p>');
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(3);
  });

  it('handles multiple paragraphs, decorating spaces in each independently', () => {
    const ed = makeEditor('<p>a b</p><p>c d e</p>');
    ed.commands.setInvisibles(true);
    expect(ed.view.dom.querySelectorAll('.inv-space').length).toBe(3);
  });
});
