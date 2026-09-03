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
  protected: boolean;
  /** Plaintext title (first line of content) — stays visible even when
   *  `content` is sealed for a protected note, so the note stays findable. */
  title: string;
  /** "Hide from MCP" opt-out — local only, independent of `protected`. */
  mcpHidden: boolean;
}

export interface Folder {
  id: string;
  name: string;
  parentId: string | null;
  position: number;
  icon: string;
  color: string;
  sort: string;
  locked: boolean;
  /** "Hide from MCP" opt-out — local only, independent of `locked`. */
  mcpHidden: boolean;
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

export interface VaultStatus {
  exists: boolean;
  unlocked: boolean;
  biometric: boolean;
  /**
   * The workspace already held a vault this device's own record did not
   * create. Surfaced as a warning on the Security page — nothing is blocked.
   */
  conflict: boolean;
  /**
   * Whether a recovery key exists for this user at all. An invited member is
   * handed a wrapped key but never a recovery key, so the recovery controls
   * would be a dead end for them.
   */
  recoveryHolder: boolean;
  /**
   * The workspace rotated its key and parked this member's new wrap under a
   * one-time rotation code — the unlock flow asks for it.
   */
  rotationCode: boolean;
  /**
   * This user holds the recovery key and some key generation has no recovery
   * wrap yet (somebody else rotated). Only they can add the missing one.
   */
  recoveryMissing: boolean;
  /**
   * The workspace has rotated past every key generation this device holds, so
   * the backend refuses every SEAL. Protected notes are shown read-only while
   * this is true — letting the user type into a note whose save is guaranteed
   * to be rejected would lose the edit.
   */
  sealOutdated: boolean;
  /**
   * Whether the Security page offers "create a recovery key": a server
   * workspace, this user is its owner, they hold no recovery key of their own
   * yet, and the vault is unlocked (the wraps are made from the live keys).
   */
  recoveryEligible: boolean;
}

/**
 * What creating an owner's own recovery key produced: its dash-separated
 * groups, shown exactly once. `incomplete` is true when an upload failed
 * partway through — at least one key generation already got a wrap out of
 * this key (so it must still be shown), but not every generation did. The
 * existing "add recovery key" follow-up completes the set with this same key.
 */
export interface RecoveryCreated {
  groups: string[];
  incomplete: boolean;
}

/**
 * One remaining member and the one-time code that opens their new wrap after
 * a key rotation. Shown once, to be handed over out of band — never stored.
 */
export interface RotationCode {
  userId: number;
  name: string;
  code: string;
}

/**
 * What resolving a local-vs-workspace vault conflict did: how many notes left
 * the device's own vault, and how many neither key could open.
 */
export interface ConflictOutcome {
  changed: number;
  skipped: number;
}

/**
 * One re-coded invitation and its fresh one-time code.
 * Shown once, to be handed over out of band — never stored.
 */
export interface RecodedInvite {
  invitationId: number;
  code: string;
}
