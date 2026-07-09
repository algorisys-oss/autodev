/** Decode a base64 string (as sent by the Rust core for PTY output) into bytes.
 *  Kept separate and pure so it can be unit-tested without a DOM/terminal. */
export function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
