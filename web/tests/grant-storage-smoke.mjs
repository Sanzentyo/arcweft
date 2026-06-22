import {
  ApiGrantStore,
  IndexedDbGrantStore,
  OpfsGrantStore,
  ReplicatedGrantStore,
} from "../arcweft-grant-storage.js";

const required = [OpfsGrantStore, IndexedDbGrantStore, ApiGrantStore, ReplicatedGrantStore];
for (const item of required) {
  if (typeof item !== "function") {
    throw new Error("grant storage export is not a class constructor");
  }
}

class MemoryStore {
  constructor() {
    this.records = new Map();
  }

  async load(options = {}) {
    return [...this.records.values()].filter((record) => {
      return options.includeTombstones === true || record.tombstone !== true;
    });
  }

  async persist(record) {
    this.records.set(record.id, { ...record });
  }

  async revoke(id) {
    this.records.set(id, { id, tombstone: true, revision: 2, updatedAtUnixMillis: 2 });
  }
}

const first = new MemoryStore();
const second = new MemoryStore();
await first.persist({ id: "grant-a", revision: 1, updatedAtUnixMillis: 1 });
const replicated = new ReplicatedGrantStore([first, second]);
const grants = await replicated.load();
if (grants.length !== 1 || grants[0].id !== "grant-a") {
  throw new Error("replicated store did not merge active grant records");
}
if ((await second.load()).length !== 1) {
  throw new Error("replicated store did not copy merged records to all targets");
}
await replicated.revoke("grant-a");
if ((await replicated.load()).length !== 0) {
  throw new Error("replicated store did not hide tombstoned grants");
}

console.log(JSON.stringify({ exports: required.map((item) => item.name).sort() }));
