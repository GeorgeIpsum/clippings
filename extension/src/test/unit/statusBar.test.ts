import * as assert from 'node:assert/strict';
import type { StatusParams } from '../../protocol';
import { nextStatusBarMode, OnceNotice, statusBarView } from '../../status/presentation';

const idle: StatusParams = {
  instance: 'i',
  scanning: false,
  interrupted: false,
  needsScan: false,
  error: null,
  warnings: [],
  statusBar: { text: '$(check) 3', tooltip: 'Clippings total', visible: true },
  badge: { value: 0, tooltip: '0 todos' },
  viewTitle: 'Tree',
  hasSubTags: false,
  isEmpty: false,
};

describe('status bar', () => {
  it('shows the server text and clicks through to the click behaviour', () => {
    assert.deepEqual(statusBarView(idle), {
      text: '$(check) 3',
      tooltip: 'Clippings total',
      visible: true,
      command: 'clippings.onStatusBarClicked',
    });
  });

  it('shows scanning, clickable to stop, even when the mode is none', () => {
    const view = statusBarView({ ...idle, scanning: true, statusBar: { text: '', tooltip: '', visible: false } });
    assert.deepEqual(view, {
      text: 'Clippings: Scanning...',
      tooltip: 'Click to interrupt scan',
      command: 'clippings.stopScan',
      visible: true,
    });
  });

  it('shows an interrupted scan, clickable to refresh', () => {
    const view = statusBarView({ ...idle, interrupted: true });
    assert.equal(view.text, 'Clippings: Scanning interrupted.');
    assert.equal(view.command, 'clippings.refresh');
  });

  it('cycles total, tags, top three, current file with todo-tree’s messages', () => {
    const seen: string[] = [];
    let mode = nextStatusBarMode('none');
    seen.push(mode.message);
    for (let i = 0; i < 4; i++) {
      mode = nextStatusBarMode(mode.mode);
      seen.push(`${mode.mode}: ${mode.message}`);
    }
    assert.deepEqual(seen, [
      'Clippings: Now showing total tags',
      'tags: Clippings: Now showing tag counts',
      'top three: Clippings: Now showing top three tag counts',
      'current file: Clippings: Now showing total tags in current file',
      'total: Clippings: Now showing total tags',
    ]);
  });
});

describe('once notices', () => {
  it('shows each distinct set once, in any order', () => {
    const notice = new OnceNotice();
    assert.equal(notice.next([]), undefined);
    assert.equal(notice.next(['b', 'a']), 'Clippings: b; a');
    assert.equal(notice.next(['a', 'b']), undefined);
    assert.equal(notice.next(['a']), 'Clippings: a');
    assert.equal(notice.next([]), undefined);
    assert.equal(notice.next(['a']), 'Clippings: a', 'shown again after it cleared');
  });
});
