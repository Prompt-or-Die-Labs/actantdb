import { randomBytes } from "node:crypto";

const ENCODING = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const TIME_LEN = 10;
const RANDOM_LEN = 16;

function encodeTime(now: number, len: number): string {
  let out = "";
  for (let i = len - 1; i >= 0; i--) {
    const mod = now % 32;
    out = ENCODING[mod] + out;
    now = (now - mod) / 32;
  }
  return out;
}

let lastTime = 0;
let lastRandom: number[] = new Array<number>(RANDOM_LEN).fill(0);

function freshRandom(): number[] {
  const bytes = randomBytes(RANDOM_LEN);
  const out: number[] = new Array<number>(RANDOM_LEN);
  for (let i = 0; i < RANDOM_LEN; i++) out[i] = bytes[i]! % 32;
  return out;
}

function incrementRandom(arr: number[]): number[] {
  const out = arr.slice();
  for (let i = out.length - 1; i >= 0; i--) {
    if (out[i]! < 31) {
      out[i] = out[i]! + 1;
      return out;
    }
    out[i] = 0;
  }
  // Overflow: regenerate.
  return freshRandom();
}

/** Generate a Crockford-base32 ULID with monotonic ordering within ms. */
export function ulid(now: number = Date.now()): string {
  let randomPart: number[];
  if (now === lastTime) {
    randomPart = incrementRandom(lastRandom);
  } else {
    lastTime = now;
    randomPart = freshRandom();
  }
  lastRandom = randomPart;
  const random = randomPart.map((c) => ENCODING[c]!).join("");
  return encodeTime(now, TIME_LEN) + random;
}
