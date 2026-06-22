const DEFAULT_DATABASE = "arcweft-persistent-grants";
const DEFAULT_STORE = "grants";
const DEFAULT_OPFS_FILE = "arcweft-persistent-grants.json";

export class OpfsGrantStore {
  constructor(options = {}) {
    this.fileName = options.fileName ?? DEFAULT_OPFS_FILE;
    this.storage = options.storage ?? globalThis.navigator?.storage;
  }

  async load(options = {}) {
    const records = await this.#readRecords();
    return filterRecords(records, options);
  }

  async persist(record) {
    const records = await this.#readRecords();
    records.set(record.id, normalizeRecord(record));
    await this.#writeRecords(records);
  }

  async revoke(id) {
    const records = await this.#readRecords();
    records.set(id, tombstone(id));
    await this.#writeRecords(records);
  }

  async #readRecords() {
    const root = await this.#root();
    try {
      const handle = await root.getFileHandle(this.fileName);
      const file = await handle.getFile();
      const text = await file.text();
      return recordsFromArray(JSON.parse(text || "[]"));
    } catch (error) {
      if (error?.name === "NotFoundError") {
        return new Map();
      }
      throw error;
    }
  }

  async #writeRecords(records) {
    const root = await this.#root();
    const handle = await root.getFileHandle(this.fileName, { create: true });
    const writable = await handle.createWritable();
    await writable.write(JSON.stringify([...records.values()], null, 2));
    await writable.close();
  }

  async #root() {
    if (!this.storage?.getDirectory) {
      throw new Error("OPFS is not available in this browser context");
    }
    return await this.storage.getDirectory();
  }
}

export class IndexedDbGrantStore {
  constructor(options = {}) {
    this.databaseName = options.databaseName ?? DEFAULT_DATABASE;
    this.storeName = options.storeName ?? DEFAULT_STORE;
    this.indexedDB = options.indexedDB ?? globalThis.indexedDB;
  }

  async load(options = {}) {
    const db = await this.#database();
    const records = await requestToPromise(
      transactionStore(db, this.storeName, "readonly").getAll(),
    );
    db.close();
    return filterRecords(records.map(normalizeRecord), options);
  }

  async persist(record) {
    const db = await this.#database();
    await requestToPromise(
      transactionStore(db, this.storeName, "readwrite").put(normalizeRecord(record)),
    );
    db.close();
  }

  async revoke(id) {
    await this.persist(tombstone(id));
  }

  async #database() {
    if (!this.indexedDB?.open) {
      throw new Error("IndexedDB is not available in this browser context");
    }
    const request = this.indexedDB.open(this.databaseName, 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore(this.storeName, { keyPath: "id" });
    };
    return await requestToPromise(request);
  }
}

export class ApiGrantStore {
  constructor(options = {}) {
    this.endpoint = options.endpoint ?? "/arcweft/grants";
    this.fetch = options.fetch ?? globalThis.fetch;
  }

  async load(options = {}) {
    const response = await this.#fetch(this.endpoint, { method: "GET" });
    const body = await response.json();
    const records = Array.isArray(body) ? body : body.grants;
    return filterRecords((records ?? []).map(normalizeRecord), options);
  }

  async persist(record) {
    const normalized = normalizeRecord(record);
    await this.#fetch(`${this.endpoint}/${encodeURIComponent(normalized.id)}`, {
      method: "PUT",
      headers: conditionalHeaders(normalized),
      body: JSON.stringify(normalized),
    });
  }

  async revoke(id) {
    const record = tombstone(id);
    await this.#fetch(`${this.endpoint}/${encodeURIComponent(record.id)}`, {
      method: "PUT",
      headers: { "content-type": "application/json", "x-arcweft-tombstone": "true" },
      body: JSON.stringify(record),
    });
  }

  async #fetch(input, init) {
    if (typeof this.fetch !== "function") {
      throw new Error("Fetch API is not available in this browser context");
    }
    const response = await this.fetch(input, init);
    if (!response.ok) {
      throw new Error(`grant API request failed with HTTP ${response.status}`);
    }
    return response;
  }
}

export class ReplicatedGrantStore {
  constructor(stores) {
    if (!Array.isArray(stores) || stores.length === 0) {
      throw new Error("ReplicatedGrantStore requires at least one target store");
    }
    this.stores = stores;
  }

  async load(options = {}) {
    const merged = new Map();
    for (const store of this.stores) {
      for (const record of await store.load({ includeTombstones: true })) {
        const normalized = normalizeRecord(record);
        const previous = merged.get(normalized.id);
        if (!previous || compareRecordVersion(previous, normalized) < 0) {
          merged.set(normalized.id, normalized);
        }
      }
    }
    const records = [...merged.values()];
    await Promise.all(this.stores.map((store) => replicate(store, records)));
    return filterRecords(records, options);
  }

  async persist(record) {
    const normalized = normalizeRecord(record);
    await Promise.all(this.stores.map((store) => store.persist(normalized)));
  }

  async revoke(id) {
    const record = tombstone(id);
    await Promise.all(this.stores.map((store) => store.revoke(record.id)));
  }
}

async function replicate(store, records) {
  await Promise.all(records.map((record) => store.persist(record)));
}

function recordsFromArray(records) {
  return new Map((records ?? []).map((record) => {
    const normalized = normalizeRecord(record);
    return [normalized.id, normalized];
  }));
}

function filterRecords(records, options) {
  const includeTombstones = options.includeTombstones === true;
  return records.filter((record) => includeTombstones || !record.tombstone);
}

function normalizeRecord(record) {
  if (!record || typeof record.id !== "string" || record.id.length === 0) {
    throw new Error("grant record requires a non-empty string id");
  }
  return {
    ...record,
    revision: Number.isSafeInteger(record.revision) ? record.revision : 0,
    updatedAtUnixMillis: Number.isSafeInteger(record.updatedAtUnixMillis)
      ? record.updatedAtUnixMillis
      : 0,
    tombstone: record.tombstone === true,
  };
}

function tombstone(id) {
  return normalizeRecord({
    id,
    revision: 0,
    updatedAtUnixMillis: Date.now(),
    tombstone: true,
  });
}

function compareRecordVersion(left, right) {
  return (
    left.revision - right.revision
    || left.updatedAtUnixMillis - right.updatedAtUnixMillis
    || left.id.localeCompare(right.id)
  );
}

function conditionalHeaders(record) {
  const headers = { "content-type": "application/json" };
  if (typeof record.etag === "string" && record.etag.length > 0) {
    headers["if-match"] = record.etag;
  }
  return headers;
}

function transactionStore(db, storeName, mode) {
  return db.transaction(storeName, mode).objectStore(storeName);
}

function requestToPromise(request) {
  return new Promise((resolve, reject) => {
    request.onerror = () => reject(request.error);
    request.onsuccess = () => resolve(request.result);
  });
}
