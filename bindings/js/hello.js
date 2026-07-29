/**
 * hello.js — Node.js Hello World for zenoh-over-iroh
 *
 * 运行:
 *   cd bindings/js
 *   npm install koffi
 *   node hello.js
 */

const zenoh = require('./zenoh.js');

console.log('=== zenoh-over-iroh JS Hello World ===\n');

// 1. Open session
const cfg = JSON.stringify({
  mode: 'peer',
  listen: { endpoints: ['tcp/127.0.0.1:0'] },
  scouting: { multicast: { enabled: false } }
});

console.log('Opening session...');
zenoh.open(cfg);
console.log('✅ open()');

// 2. Put
const ok = zenoh.put('demo/test', 'hello from JS');
console.log(`✅ put("demo/test", "hello from JS") → ${ok}`);

// 3. Get ZID
const id = zenoh.zid();
console.log(`✅ zid() → ${id}`);

// 4. Close
zenoh.close();
console.log('✅ close()\n');

console.log('=== ALL PASS ===');
