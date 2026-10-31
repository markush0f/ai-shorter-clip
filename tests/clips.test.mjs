import test from 'node:test';
import assert from 'node:assert/strict';
import { createDemoClips, formatTime } from '../src/clips.ts';

test('los intervalos son válidos, deterministas y duran como máximo 30 segundos', () => {
  for (const duration of [0.01, 0.5, 1, 1.9, 2, 2.9, 3, 6, 91, 36000]) {
    const clips = createDemoClips(duration);
    assert.ok(clips.length >= 1 && clips.length <= 3);
    assert.deepEqual(clips, createDemoClips(duration));
    for (const clip of clips) {
      assert.ok(clip.startSeconds >= 0);
      assert.ok(clip.endSeconds <= duration);
      assert.ok(clip.endSeconds > clip.startSeconds);
      assert.ok(clip.endSeconds - clip.startSeconds <= 30.000001);
      if (duration >= 1) assert.ok(clip.endSeconds - clip.startSeconds >= 1);
    }
  }
});

test('descarta duraciones inválidas', () => {
  for (const duration of [0, -1, NaN, Infinity]) assert.deepEqual(createDemoClips(duration), []);
});

test('formatea minutos, horas y vídeos inferiores a un segundo', () => {
  assert.equal(formatTime(0), '0:00');
  assert.equal(formatTime(65), '1:05');
  assert.equal(formatTime(3601), '1:00:01');
  assert.equal(formatTime(0.5), '0,5 s');
});
