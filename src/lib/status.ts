import { createSignal } from "solid-js";

/** Transient app-wide voice status shown in the footer. `recording` gets a red record-dot,
 *  `working` (transcribing / model download) gets a spinner. `null` hides the slot. Written by
 *  whoever drives the action (the composer), read by the StatusFooter — module-level so the two
 *  share it without prop threading, mirroring `extensions.ts`. */
export type VoiceStatus = { text: string; kind: "recording" | "working" } | null;

const [voiceStatus, setVoiceStatus] = createSignal<VoiceStatus>(null);

export { voiceStatus, setVoiceStatus };
