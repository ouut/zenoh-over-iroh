/**
 * zenoh.js — Node.js binding for libzenoh_over_iroh.so
 * Uses koffi (modern C FFI) — no compilation needed.
 *
 * 前置条件:
 *   npm install koffi
 *   # libzenoh_over_iroh.so 在 LD_LIBRARY_PATH 或 bondings/ 目录下
 *
 * 使用:
 *   const zenoh = require('./zenoh.js');
 *   const s = zenoh.open('{"listen":{"endpoints":["tcp/127.0.0.1:0"]}}');
 *   zenoh.put(s, 'hello', 'world');
 *   zenoh.close(s);
 */

const koffi = require('koffi');
const path = require('path');

let lib = null;

function findLib() {
  // Try common locations
  const candidates = [
    path.join(__dirname, '../../target/release/libzenoh_over_iroh.so'),
    path.join(__dirname, '../../target/debug/libzenoh_over_iroh.so'),
    process.env.ZENOH_LIB,
    'libzenoh_over_iroh.so',
  ];
  for (const c of candidates) {
    if (!c) continue;
    try { require('fs').accessSync(c); return koffi.load(c); } catch(_) {}
  }
  throw new Error(`Cannot find libzenoh_over_iroh.so. Tried: ${candidates.filter(Boolean).join(', ')}`);
}

function load() {
  if (lib) return lib;
  lib = findLib();

  // Declare C functions
  lib.func('z_open', 'int', ['str']);
  lib.func('z_put', 'int', ['str', 'str']);
  lib.func('z_close', 'int', []);
  lib.func('z_zid', 'string', []);
  lib.func('z_free_string', 'void', ['string']);

  return lib;
}

module.exports = {
  open(configJson) {
    const l = load();
    const ret = l.z_open(configJson);
    if (ret !== 0) throw new Error(`z_open failed: ${ret}`);
    return true;
  },

  put(key, value) {
    const l = load();
    return l.z_put(key, value) === 0;
  },

  close() {
    const l = load();
    l.z_close();
  },

  zid() {
    const l = load();
    const id = l.z_zid();
    if (id) l.z_free_string(id);
    return id;
  }
};
