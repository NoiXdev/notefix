import { describe, it, expect } from 'vitest';
import { sortNotesBy } from './sortNotes';
import type { NoteMeta } from './types';
import { getPreview } from './preview';

const n = (id: string, content: string, o: Partial<NoteMeta> = {}): NoteMeta =>
  ({ id, preview: getPreview(content), tasksDone: 0, tasksTotal: 0, updatedAt: 0, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null, ...o });

describe('sortNotesBy', () => {
  it('manual sorts by position, pinned first', () => {
    const notes = [n('a', 'A', { position: 2 }), n('b', 'B', { position: 1, pinned: true }), n('c', 'C', { position: 0 })];
    expect(sortNotesBy(notes, 'manual').map(x => x.id)).toEqual(['b', 'c', 'a']);
  });
  it('titleAsc/Desc by preview text', () => {
    const notes = [n('a', '<p>Banana</p>'), n('b', '<p>Apple</p>')];
    expect(sortNotesBy(notes, 'titleAsc').map(x => x.id)).toEqual(['b', 'a']);
    expect(sortNotesBy(notes, 'titleDesc').map(x => x.id)).toEqual(['a', 'b']);
  });
  it('updated asc/desc', () => {
    const notes = [n('a', 'A', { updatedAt: 1 }), n('b', 'B', { updatedAt: 2 })];
    expect(sortNotesBy(notes, 'updatedDesc').map(x => x.id)).toEqual(['b', 'a']);
    expect(sortNotesBy(notes, 'updatedAsc').map(x => x.id)).toEqual(['a', 'b']);
  });
  it('dueAsc puts nulls last', () => {
    const notes = [n('a', 'A', { dueAt: null }), n('b', 'B', { dueAt: 500 }), n('c', 'C', { dueAt: 100 })];
    expect(sortNotesBy(notes, 'dueAsc').map(x => x.id)).toEqual(['c', 'b', 'a']);
  });
  it('unknown value falls back to manual', () => {
    const notes = [n('a', 'A', { position: 1 }), n('b', 'B', { position: 0 })];
    expect(sortNotesBy(notes, 'whatever').map(x => x.id)).toEqual(['b', 'a']);
  });
});
