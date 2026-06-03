import { createWriteStream, mkdirSync } from "node:fs";
import { once } from "node:events";
import { join } from "node:path";

const SAMPLE_RATE = 48_000;
const CHANNELS = 2;
const SECONDS = 5 * 60;
const TRACKS = 8;
const FRAMES_PER_CHUNK = SAMPLE_RATE;
const OUTPUT_DIR = "/tmp/thesia-benchmark";

function createHeader(dataBytes) {
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + dataBytes, 4);
  header.write("WAVE", 8);
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20);
  header.writeUInt16LE(CHANNELS, 22);
  header.writeUInt32LE(SAMPLE_RATE, 24);
  header.writeUInt32LE(SAMPLE_RATE * CHANNELS * 2, 28);
  header.writeUInt16LE(CHANNELS * 2, 32);
  header.writeUInt16LE(16, 34);
  header.write("data", 36);
  header.writeUInt32LE(dataBytes, 40);
  return header;
}

function createChunk(trackIndex, startFrame, frameCount) {
  const chunk = Buffer.alloc(frameCount * CHANNELS * 2);
  const baseFrequency = 110 + trackIndex * 27;
  for (let i = 0; i < frameCount; i += 1) {
    const frame = startFrame + i;
    for (let channel = 0; channel < CHANNELS; channel += 1) {
      const frequency = baseFrequency * (channel === 0 ? 1 : 1.5);
      const value = Math.sin((2 * Math.PI * frequency * frame) / SAMPLE_RATE) * 0.5;
      chunk.writeInt16LE(Math.round(value * 32767), (i * CHANNELS + channel) * 2);
    }
  }
  return chunk;
}

async function writeTrack(trackIndex) {
  const frameCount = SAMPLE_RATE * SECONDS;
  const dataBytes = frameCount * CHANNELS * 2;
  const path = join(OUTPUT_DIR, `benchmark-${trackIndex + 1}.wav`);
  const stream = createWriteStream(path);
  stream.write(createHeader(dataBytes));
  for (let frame = 0; frame < frameCount; frame += FRAMES_PER_CHUNK) {
    const chunkFrames = Math.min(FRAMES_PER_CHUNK, frameCount - frame);
    if (!stream.write(createChunk(trackIndex, frame, chunkFrames))) await once(stream, "drain");
  }
  stream.end();
  await once(stream, "finish");
  console.log(path);
}

mkdirSync(OUTPUT_DIR, { recursive: true });
for (let trackIndex = 0; trackIndex < TRACKS; trackIndex += 1) {
  await writeTrack(trackIndex);
}
