export interface Note {
  id: string;
  content: string; // Tiptap HTML
  updatedAt: number;
  pinned: boolean;
  archived: boolean;
  color: string;
  dueAt: number | null;
  folderId: string | null;
  position: number;
  deletedAt: number | null;
}

/** Lightweight list item: every note field except the (potentially huge) HTML
 *  content, plus a short preview + task counts. Content is loaded on demand. */
export interface NoteMeta {
  id: string;
  updatedAt: number;
  pinned: boolean;
  archived: boolean;
  color: string;
  dueAt: number | null;
  folderId: string | null;
  position: number;
  deletedAt: number | null;
  preview: string;
  tasksDone: number;
  tasksTotal: number;
}

export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
  position: number;
  icon: string;
  color: string;
  sort: string;
}

export interface Stats {
  notes: number;
  archived: number;
  characters: number;
  words: number;
}

export interface Revision {
  id: number;
  noteId: string;
  createdAt: number;
}
