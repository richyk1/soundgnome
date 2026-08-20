/**
 * Persistent (IndexedDB) cache for normalised waveform peaks, keyed by the peaks
 * or audio URL. Complements the in-memory session cache in `Waveform.svelte`:
 * once a track's peaks are known (fetched from the server or computed locally),
 * they survive reloads, so replaying it is instant with zero network or decode.
 *
 * Everything is best-effort: any IndexedDB error degrades to a cache miss rather
 * than breaking the scrubber.
 *
 * Note: uses the `new Promise` executor form rather than `Promise.withResolvers`
 * because the web tsconfig targets es2023, which predates it.
 */

const DB_NAME = 'soundgnome';
const STORE = 'peaks';
const VERSION = 1;

let dbPromise: Promise<IDBDatabase> | null = null;

function open(): Promise<IDBDatabase> {
  if (typeof indexedDB === 'undefined') {
    return Promise.reject(new Error('IndexedDB unavailable'));
  }
  if (dbPromise) return dbPromise;
  dbPromise = new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, VERSION);
    req.onupgradeneeded = () => {
      if (!req.result.objectStoreNames.contains(STORE)) {
        req.result.createObjectStore(STORE);
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
  return dbPromise;
}

// Our own persisted data (we only ever store `number[]`); a cheap head check is
// enough to reject a corrupt/foreign record without scanning 900 elements.
function isPeaks(v: unknown): v is number[] {
  return Array.isArray(v) && v.length > 0 && typeof v[0] === 'number';
}

/** Cached peaks for `key`, or `null` on a miss or any error. */
export async function getCachedPeaks(key: string): Promise<number[] | null> {
  try {
    const db = await open();
    return await new Promise<number[] | null>((resolve) => {
      const tx = db.transaction(STORE, 'readonly');
      const req = tx.objectStore(STORE).get(key);
      req.onsuccess = () => {
        const v: unknown = req.result;
        resolve(isPeaks(v) ? v : null);
      };
      req.onerror = () => resolve(null);
    });
  } catch {
    return null;
  }
}

/** Persist `peaks` under `key`. Fire-and-forget; failures are ignored. */
export async function putCachedPeaks(key: string, peaks: number[]): Promise<void> {
  if (!peaks.length) return;
  try {
    const db = await open();
    const tx = db.transaction(STORE, 'readwrite');
    tx.objectStore(STORE).put(peaks, key);
  } catch {
    /* best-effort */
  }
}
