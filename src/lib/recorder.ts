export interface Recorder {
  /** Stop recording, release the mic, and resolve with the captured audio. */
  stop: () => Promise<Blob>;
}

/** Start recording from the default microphone via MediaRecorder. Rejects if mic access
 *  is denied or unavailable. */
export async function startRecording(): Promise<Recorder> {
  const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
  const chunks: BlobPart[] = [];
  const mr = new MediaRecorder(stream);
  mr.ondataavailable = (e) => {
    if (e.data.size) chunks.push(e.data);
  };
  mr.start();
  return {
    stop: () =>
      new Promise<Blob>((resolve) => {
        mr.onstop = () => {
          stream.getTracks().forEach((t) => t.stop());
          resolve(new Blob(chunks, { type: mr.mimeType || "audio/webm" }));
        };
        mr.stop();
      }),
  };
}

/** Best-effort file extension from a MediaRecorder blob's MIME type. */
export function extFromMime(mime: string): string {
  const m = /audio\/([a-z0-9]+)/i.exec(mime);
  return m ? m[1] : "webm";
}
