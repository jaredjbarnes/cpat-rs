// Small deterministic helpers for building workload inputs. Every input is a
// pure function of the corpus and a fixed seed, so every run (and every
// engine) gets byte-for-byte the same text.

export const MB = 1024 * 1024;

/** mulberry32: a tiny seeded PRNG, so generated text never changes. */
export function random(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export const pick = (next, list) => list[Math.floor(next() * list.length)];

export const utf8Bytes = text => Buffer.byteLength(text, "utf8");

/** `unit` joined with `separator` until the result reaches `bytes` UTF-8
 * bytes. Always a whole number of units, so a unit that parses keeps
 * parsing when repeated. */
export function repeatTo(unit, separator, bytes) {
  const parts = [unit];
  let size = utf8Bytes(unit);
  const step = utf8Bytes(separator) + size;
  while (size < bytes) {
    parts.push(unit);
    size += step;
  }
  return parts.join(separator);
}

/** `count` items spread evenly over `list` (all of it when shorter). */
export const spreadOver = (list, count) =>
  list.length <= count ? list : Array.from({ length: count }, (_, i) => list[Math.floor((i * list.length) / count)]);
