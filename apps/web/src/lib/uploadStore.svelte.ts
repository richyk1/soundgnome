/**
 * Browser upload manager for the Ingest page.
 *
 * Two phases: upload (client-driven, bounded concurrency, per-file progress) then
 * ingest (a server background task that dedups + files each track). A singleton so
 * an in-flight upload survives navigation, like the audio player.
 */
import { uploadFile, ingestSession, getTasks } from './api';
import type { TaskDto } from './types';

const AUDIO_EXT = ['mp3', 'flac', 'm4a', 'mp4', 'aac', 'ogg', 'opus', 'wav'];
const CONCURRENCY = 4;

export type ItemState = 'queued' | 'uploading' | 'uploaded' | 'error';
export type Phase = 'idle' | 'uploading' | 'ingesting' | 'done';

export interface UploadItem {
  id: string;
  file: File;
  relativePath: string;
  size: number;
  state: ItemState;
  loaded: number; // bytes uploaded so far
  error: string | null;
}

/** `crypto.randomUUID` needs a secure context; fall back for plain-HTTP dev boxes. */
function uuid(): string {
  const c = globalThis.crypto;
  if (c && typeof c.randomUUID === 'function') {
    try {
      return c.randomUUID();
    } catch {
      /* fall through */
    }
  }
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (ch) => {
    const r = (Math.random() * 16) | 0;
    return (ch === 'x' ? r : (r & 0x3) | 0x8).toString(16);
  });
}

export function isAudioFile(name: string): boolean {
  const dot = name.lastIndexOf('.');
  return dot >= 0 && AUDIO_EXT.includes(name.slice(dot + 1).toLowerCase());
}

class UploadManager {
  session = uuid();
  items = $state<UploadItem[]>([]);
  phase = $state<Phase>('idle');
  ingestError = $state<string | null>(null);
  ingestTaskId = $state<number | null>(null);
  ingestTask = $state<TaskDto | null>(null);

  #aborts = new Map<string, () => void>();
  #cancelled = false;
  #pollTimer: number | null = null;

  // ── Derived aggregates ─────────────────────────────────────────────────────
  get total(): number {
    return this.items.length;
  }
  get uploadedCount(): number {
    return this.items.filter((i) => i.state === 'uploaded').length;
  }
  get uploadingCount(): number {
    return this.items.filter((i) => i.state === 'uploading').length;
  }
  get errorCount(): number {
    return this.items.filter((i) => i.state === 'error').length;
  }
  get totalBytes(): number {
    return this.items.reduce((s, i) => s + i.size, 0);
  }
  get uploadedBytes(): number {
    return this.items.reduce((s, i) => s + (i.state === 'uploaded' ? i.size : i.loaded), 0);
  }
  get bytePct(): number {
    return this.totalBytes === 0 ? 0 : Math.round((this.uploadedBytes / this.totalBytes) * 100);
  }
  get uploading(): UploadItem[] {
    return this.items.filter((i) => i.state === 'uploading');
  }
  get errored(): UploadItem[] {
    return this.items.filter((i) => i.state === 'error');
  }

  /** Add selected/dropped files. Filters to audio and de-dups the selection.
   *  Returns counts so the caller can surface what was added vs skipped. */
  addFiles(entries: { file: File; relativePath: string }[]): {
    added: number;
    skippedNonAudio: number;
    skippedDuplicate: number;
  } {
    const seen = new Set(this.items.map((i) => `${i.relativePath}:${i.size}`));
    let added = 0;
    let skippedNonAudio = 0;
    let skippedDuplicate = 0;
    for (const { file, relativePath } of entries) {
      if (!isAudioFile(file.name)) {
        skippedNonAudio++;
        continue;
      }
      const key = `${relativePath}:${file.size}`;
      if (seen.has(key)) {
        skippedDuplicate++;
        continue;
      }
      seen.add(key);
      this.items.push({
        id: uuid(),
        file,
        relativePath,
        size: file.size,
        state: 'queued',
        loaded: 0,
        error: null,
      });
      added++;
    }
    return { added, skippedNonAudio, skippedDuplicate };
  }

  reset() {
    this.cancel();
    this.items = [];
    this.session = uuid();
    this.phase = 'idle';
    this.ingestError = null;
    this.ingestTaskId = null;
    this.ingestTask = null;
    this.#cancelled = false;
  }

  cancel() {
    this.#cancelled = true;
    for (const abort of this.#aborts.values()) abort();
    this.#aborts.clear();
    if (this.#pollTimer) {
      clearInterval(this.#pollTimer);
      this.#pollTimer = null;
    }
    if (this.phase === 'uploading') this.phase = 'idle';
  }

  async start() {
    if (this.phase === 'uploading' || this.phase === 'ingesting') return;
    if (!this.items.some((i) => i.state === 'queued')) return;
    this.#cancelled = false;
    this.ingestError = null;
    this.phase = 'uploading';

    const workers = Array.from({ length: CONCURRENCY }, () => this.#worker());
    await Promise.all(workers);

    if (this.#cancelled) return;
    if (this.items.some((i) => i.state === 'uploaded')) {
      await this.#startIngest();
    } else {
      this.phase = 'idle';
    }
  }

  retryFailed() {
    for (const i of this.items) {
      if (i.state === 'error') {
        i.state = 'queued';
        i.error = null;
        i.loaded = 0;
      }
    }
    this.start();
  }

  async #worker() {
    while (!this.#cancelled) {
      // No await between find and state mutation, so no two workers grab the same item.
      const item = this.items.find((i) => i.state === 'queued');
      if (!item) return;
      item.state = 'uploading';
      item.loaded = 0;

      const { promise, abort } = uploadFile(this.session, item.relativePath, item.file, (loaded) => {
        item.loaded = loaded;
      });
      this.#aborts.set(item.id, abort);
      try {
        await promise;
        item.state = 'uploaded';
        item.loaded = item.size;
      } catch (e) {
        if (this.#cancelled) return;
        item.state = 'error';
        item.error = e instanceof Error ? e.message : String(e);
      } finally {
        this.#aborts.delete(item.id);
      }
    }
  }

  async #startIngest() {
    this.phase = 'ingesting';
    try {
      const { task_id } = await ingestSession(this.session);
      this.ingestTaskId = task_id;
      this.#pollTimer = setInterval(() => this.#poll(), 1500);
      await this.#poll();
    } catch (e) {
      this.ingestError = e instanceof Error ? e.message : String(e);
      this.phase = 'done';
    }
  }

  async #poll() {
    if (this.ingestTaskId == null) return;
    try {
      const tasks = await getTasks();
      const t = tasks.find((x) => x.id === this.ingestTaskId) ?? null;
      this.ingestTask = t;
      if (t && (t.status === 'Completed' || t.status === 'Failed' || t.status === 'Cancelled')) {
        if (this.#pollTimer) {
          clearInterval(this.#pollTimer);
          this.#pollTimer = null;
        }
        this.phase = 'done';
      }
    } catch {
      /* transient poll error; keep polling */
    }
  }
}

export const uploadManager = new UploadManager();
