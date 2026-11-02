import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { undoDepth } from '@tiptap/pm/history';
import { UndoGrouping, clearUndoHistory, startsWord } from './undoGrouping';
import { SearchHighlight, searchState } from './searchHighlight';

const editors: Editor[] = [];
function make(extensions = [StarterKit, UndoGrouping], content = '<p></p>') {
  const ed = new Editor({ extensions, content });
  editors.push(ed);
  return ed;
}
afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; });

/** Types char by char with no pause, the way a fast typist does. */
function typeFast(ed: Editor, text: string) {
  for (const ch of text) {
    const { from } = ed.state.selection;
    ed.view.dispatch(ed.state.tr.insertText(ch, from).setTime(1000));
  }
}

describe('startsWord (pure)', () => {
  const ed = () => make([StarterKit], '<p>ab cd</p>');
  it('is true at the start of a textblock', () => expect(startsWord(ed().state.doc, 1, 'x')).toBe(true));
  it('is true right after a space', () => expect(startsWord(ed().state.doc, 4, 'x')).toBe(true));
  it('is false inside a word', () => expect(startsWord(ed().state.doc, 2, 'x')).toBe(false));
  it('is false for whitespace', () => expect(startsWord(ed().state.doc, 4, ' ')).toBe(false));
  it('is false outside a textblock', () => expect(startsWord(ed().state.doc, 0, 'x')).toBe(false));
});

describe('UndoGrouping', () => {
  it('without it, one undo removes everything typed fast (the reported bug)', () => {
    const ed = make([StarterKit]);
    typeFast(ed, 'hello brave world');
    ed.commands.undo();
    expect(ed.state.doc.textContent).toBe('');
  });

  it('undoes fast typing word by word', () => {
    const ed = make();
    typeFast(ed, 'hello brave world');
    ed.commands.undo();
    expect(ed.state.doc.textContent).toBe('hello brave ');
    ed.commands.undo();
    expect(ed.state.doc.textContent).toBe('hello ');
    ed.commands.redo();
    expect(ed.state.doc.textContent).toBe('hello brave ');
  });

  it('starts a new step in a new paragraph', () => {
    const ed = make();
    typeFast(ed, 'one');
    ed.commands.splitBlock();
    typeFast(ed, 'two');
    ed.commands.undo();
    expect(ed.getText()).toBe('one\n\n');
  });

  it('leaves non-typing transactions alone', () => {
    const ed = make([StarterKit, UndoGrouping], '<p>abc</p>');
    ed.commands.setTextSelection({ from: 1, to: 4 });
    ed.commands.toggleBold();
    ed.commands.undo();
    expect(ed.getHTML()).toBe('<p>abc</p>');
  });
});

describe('clearUndoHistory', () => {
  it('stops undo from reaching content replaced by a note load', () => {
    const ed = make();
    typeFast(ed, 'note A');
    ed.commands.setContent('<p>note B</p>');
    clearUndoHistory(ed);
    expect(undoDepth(ed.state)).toBe(0);
    ed.commands.undo();
    expect(ed.getHTML()).toBe('<p>note B</p>');
  });

  it("keeps other plugins' state", () => {
    const ed = make([StarterKit, UndoGrouping, SearchHighlight], '<p>find me</p>');
    ed.commands.setSearch('find');
    clearUndoHistory(ed);
    expect(searchState(ed).query).toBe('find');
  });

  it('is a no-op on an editor without a view', () => {
    expect(() => clearUndoHistory({} as Editor)).not.toThrow();
  });
});
